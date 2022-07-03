mod orchestrator;
mod leaders;
mod logger;

use std::{thread};
use std::env::args;
use std::time::Duration;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use crate::leaders::get_leader_election;
use crate::logger::Logger;

const TRACKER: &str = "tracker";
const STATUS_INFO: &str = "INFO";
const STATUS_ERROR: &str = "ERROR";
const TIME_TO_SLEEP: u64 = 1;

fn get_start_position(mut tracker_reader: BufReader<&File>) -> u64 {
    let mut start_position = 0;
    let mut buffer: String = "".to_string();
    let rb = tracker_reader.read_line(&mut buffer).unwrap();
    if rb != 0 {
        start_position = buffer.trim().parse().unwrap();
    }
    start_position
}

fn main() {
    let id: usize = args().nth(1).unwrap_or("0".to_string()).parse().unwrap(); // if this panic nmf
    if id > 9 {
        panic!("Well someone wants to break me");
    }
    let mut logger = Logger::new();
    let l = logger.clone();
    let le = get_leader_election(id, l);
    logger.change_leader();
    loop {
        let mut should_end = false;
        if le.leader() {
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
                let cl = logger.clone();
                th.push(thread::spawn(move || orchestrator::orchestrate(transaction, cl)));
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
