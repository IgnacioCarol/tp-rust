mod dead_letter;
mod leaders;
mod logger;
mod orchestrator;
mod recovery;
mod file_handler;

use core::panic;
use std::process::exit;
use dead_letter::read_dead_letter;
use file_handler::FileHandler;
use recovery::start_recovery;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::{env, thread};
use std_semaphore::Semaphore;

use crate::leaders::get_leader_election;
use crate::logger::Logger; 

const TRACKER: &str = "tracker.txt";
const STATUS_INFO: &str = "INFO";
const STATUS_ERROR: &str = "ERROR";
const TIME_TO_SLEEP: u64 = 1;
const TRANSACTIONS_FILE: &str = "transactions.csv";

fn get_start_position( tracker_file: &mut FileHandler) -> u64 {
    let mut start_position = 0;

    if let Some( pos ) = tracker_file.read(){
        start_position = pos.trim().parse().unwrap();
    }
    start_position
}

fn main() {
    // Arguments format:  script_name [--deadletter | -D]
    let mut id: usize = 100;
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--deadletter" || args[i] == "-D" {
            read_dead_letter();
            return;
        }
        if args[i] == "--id" || args[i] == "-id" {
            if i + 1 < args.len() {
                id = args[i + 1].parse().unwrap(); // if this panic nmf
                if id > 9 {
                    panic!("Well someone wants to break me");
                }
            } else {
                panic!("Is required a value for --id");
            }
        }
    }

    if id == 100 {
        panic!("argument not sent");
    }
    let mut logger = Logger::new();
    let l = logger.clone();
    let le = get_leader_election(id, l);
    logger.change_leader();
    loop {
        let mut should_end = false;
        if le.leader() {
            // Recovery proccess.
            let recovery_sem = Arc::new(Semaphore::new(1));
            start_recovery();
            // ---

            // Average total time traker
            let avg_time = Arc::new(RwLock::new((0, 0)));

            let mut result = FileHandler::new(TRACKER.to_string()) ;
            if let Err(e) = result {
                println!("Error");
                return;
            }
            let mut traker_file = result.unwrap();            

            result = FileHandler::new(TRANSACTIONS_FILE.to_string()) ;
            if let Err(e) = result {
                println!("Error");
                return;
            }
            let mut transactions_file = result.unwrap();
            
            let pointer = get_start_position(&mut traker_file);
            transactions_file.seek(pointer);
        
            let mut total_bytes_read = usize::try_from(pointer).unwrap();
            let mut th = vec![];
            while le.leader() {
               
    
                if let Some(transaction) = transactions_file.read() {
        
                    total_bytes_read += transaction.len();
                    traker_file.seek(0);
                    traker_file.write(&total_bytes_read.to_string());

                    let cl = logger.clone();
                    let sem_cl = recovery_sem.clone();
                    let avg_time_cl = avg_time.clone();
                    th.push(thread::spawn(move || {
                        orchestrator::orchestrate(
                            transaction.trim().to_string(),
                            cl,
                            sem_cl,
                            avg_time_cl,
                        )
                    }));
                    thread::sleep(Duration::from_secs(TIME_TO_SLEEP));

                }else{
                    should_end = true;
                    break;
                }
            }
            for t in th {
                t.join().unwrap();
            }
            if !le.leader() {
                logger.log_info(format!("[{}] stepping down", id));
            }
        }
        if should_end {
            break;
        }
        le.wake_me_up_when_september_ends();
    }
    logger.log(
        format!("[{}] guess this is it, nice to work for you", id).as_str(),
        STATUS_INFO,
    );
}
