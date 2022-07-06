mod orchestrator;
mod leaders;
mod logger;
pub mod recovery;

use core::panic;
use std::sync::{Arc, RwLock};
use std::{env, io, thread};
use std::time::Duration;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, stdin, Write};
use recovery::start_recovery;
use std_semaphore::Semaphore;

use crate::leaders::get_leader_election;
use crate::logger::Logger;

const TRACKER: &str = "tracker";
const STATUS_INFO: &str = "INFO";
const STATUS_ERROR: &str = "ERROR";
const TIME_TO_SLEEP: u64 = 1;
const TRANSACTIONS_FILE: &str = "transactions";
const DEADLETTER_FILE: &str = "dead_letter";

fn get_start_position(mut tracker_reader: BufReader<&File>) -> u64 {
    let mut start_position = 0;
    let mut buffer: String = "".to_string();
    let rb = tracker_reader.read_line(&mut buffer).unwrap();
    if rb != 0 {
        start_position = buffer.trim().parse().unwrap();
    }
    start_position
}

fn read_dead_letter() {
    let mut logger = Logger::new();

    let deadletter = File::options().append(false).read(true).write(true).create(true).open(DEADLETTER_FILE);
    let mut deadletter_reader;
    let deadletter_c;
    if let Ok(ref file) = deadletter {
        deadletter_reader = BufReader::new(file);
        deadletter_c  = file.try_clone().unwrap();
    }else {
        logger.log(format!("Error opening {}", DEADLETTER_FILE).as_str(), "ERROR");
        return;
    }

    let recovery_sem = Arc::new(Semaphore::new(1));

    // Average total time traker
    let avg_time = Arc::new(RwLock::new((0,0)));

    let mut deadletter_writer = BufWriter::new(&deadletter_c);
    let mut seek = 0;

    let mut line = "".to_string();
    let mut dead_transaction = String::new();
    let mut state= String::new();

    let mut input_string = String::new();
    let mut skipped = false;

    let mut read_bytes = deadletter_reader.read_line(&mut line).unwrap();
    while read_bytes > 0 && !skipped {
        
        if read_bytes >= 3 {
            state = line[read_bytes-2..read_bytes-1].to_string();
            dead_transaction = line[0..read_bytes-3].to_string();
        }

        if state == "F" {

            print!("Transacción with state {} => {} | ", state, dead_transaction);
            print!("Select an option: [P]rocess - [N]ext - [R]emove - [E]xit: ");
            io::stdout().flush().unwrap();

            loop {
                // Input from user
                input_string.clear();
                stdin().read_line(&mut input_string)
                .ok()
                .expect("Failed to read line");

              match (&input_string.to_lowercase()).as_str().trim_end() {
                    "p" => {
                        println!("Processing..");
                        let le = logger.clone();
                        let avg_time_cl = avg_time.clone();
                        let sem_cl = recovery_sem.clone();
                        orchestrator::orchestrate((&dead_transaction).to_string(), le,sem_cl,avg_time_cl);

                        deadletter_writer.seek(SeekFrom::Start(seek)).expect("should move");
                        write!(deadletter_writer,"{},P\n",dead_transaction).unwrap();
                        deadletter_writer.flush().unwrap();
                        break;
                    },
                    "n" => {
                        println!("Ignoring transaction..");
                        break;
                    },
                    "r" => {
                        println!("Removing transaction..");
                        deadletter_writer.seek(SeekFrom::Start(seek)).expect("should move");
                        write!(deadletter_writer,"{},R\n",dead_transaction).unwrap();
                        deadletter_writer.flush().unwrap();
                        break;
                    },
                    "e" => {
                        println!("Exiting the process of deadletter..");
                        skipped = true;
                        break;
                    },
                    &_ => {
                        println!("{} is not valid. Only P/N/R/E keys are valid.. ",(&input_string.to_lowercase()).as_str().trim_end() );
                    }
                }
            }
        }

        seek += read_bytes as u64;
        line.clear();
        state.clear();
        dead_transaction.clear();
        read_bytes = deadletter_reader.read_line(&mut line).unwrap();
    }
}

fn main() {
    // Arguments format:  script_name [--deadletter | -D]
    let mut id: usize = 100;
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len(){
        if args[i] == "--deadletter" || args[i] == "-D" {
            read_dead_letter();
            return;
        }
        if args[i] == "--id" || args[i] == "-id" {
            if i+1 < args.len(){
                id = args[i+1].parse().unwrap(); // if this panic nmf
                if id > 9 {
                    panic!("Well someone wants to break me");
                }
            }else{
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
            let avg_time = Arc::new(RwLock::new((0,0)));

            let tracker_file = File::options().append(false).read(true).write(true).create(true).open(TRACKER);
            let tracker_reader;
            let tf;
            if let Ok(ref file) = tracker_file {
                tracker_reader = BufReader::new(file);
                tf = file.try_clone().unwrap();
            } else {
                logger.log("Error with tracker file", STATUS_ERROR);
                return;
            }
            let mut tracker_writer = BufWriter::new(&tf);
            let pointer = get_start_position(tracker_reader);

            let transactions_file = File::open(TRANSACTIONS_FILE);
            if let Err(error) = transactions_file {
                logger.log(format!("Error opening {} {}", TRANSACTIONS_FILE, error).as_str(), STATUS_ERROR);
                return;
            }
            let mut transactions_reader = BufReader::new(transactions_file.unwrap());
            transactions_reader.seek(SeekFrom::Start(pointer)).expect("Seeked incorrectly");
            let mut total_bytes_read = usize::try_from(pointer).unwrap();
            let mut th = vec!();
            while le.leader() {
                let mut transaction = "".to_string();
                let bytes_read = transactions_reader.read_line(&mut transaction).unwrap();
                if bytes_read == 0 {
                    should_end = true;
                    break;
                }
                total_bytes_read += bytes_read;
                tracker_writer.seek(SeekFrom::Start(0)).unwrap();
                write!(tracker_writer, "{}", total_bytes_read).unwrap();
                tracker_writer.flush().unwrap();
                let cl = logger.clone();
                let sem_cl = recovery_sem.clone();
                let avg_time_cl = avg_time.clone();
                th.push(thread::spawn(move || orchestrator::orchestrate(transaction.trim().to_string(), cl,sem_cl, avg_time_cl)));
                thread::sleep(Duration::from_secs(TIME_TO_SLEEP));
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
    logger.log(format!("[{}] guess this is it, nice to work for you", id).as_str(), STATUS_INFO);
}
