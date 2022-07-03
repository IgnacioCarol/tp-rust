mod orchestrator;

use std::net::UdpSocket;
use std::sync::Barrier;
use std::sync::{Arc, RwLock};
use std::{io, thread};
use std::thread::JoinHandle;
use std::time::Duration;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};

fn id_to_ctrladdr(id: usize) -> String { "127.0.0.1:1234".to_owned() + &*id.to_string() }
fn id_to_dataaddr(id: usize) -> String { "127.0.0.1:1235".to_owned() + &*id.to_string() }
const ADDR: &str = "127.0.0.1:8000";
const TRACKER: &str = "tracker";
const TRANSACTIONS_FILE: &str = "transactions";
const TTL: Duration = Duration::from_secs(2);

struct AlGlobo {
    id: String,
    ctrl_socket: UdpSocket
//Add extra data required for election algorithm
}

impl AlGlobo {

}

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
    let mut tracker_file = File::options().append(false).read(true).write(true).create(true).open(TRACKER);
    let mut tracker_reader;
    let tf;
    if let Ok(ref file) = tracker_file {
        tracker_reader = BufReader::new(file);
        tf = file.try_clone().unwrap();
    } else {
        print!("Error with tracker file");
        return;
    }
    let mut tracker_writer = BufWriter::new(&tf);
    let mut pointer = get_start_position(tracker_reader);

    let transactions_file = File::open(TRANSACTIONS_FILE);
    if let Err(error) = transactions_file {
        println!("Error opening {} {}", TRANSACTIONS_FILE, error);
        return;
    }
    let mut transactions_reader = BufReader::new(transactions_file.unwrap());
    transactions_reader.seek(SeekFrom::Start(pointer)).expect("Seeked incorrectly");
    let mut total_bytes_read = usize::try_from(pointer).unwrap();
    loop {
        let mut transaction = "".to_string();
        let bytes_read = transactions_reader.read_line(&mut transaction).unwrap();
        if bytes_read == 0 {
            break;
        }
        total_bytes_read += bytes_read;
        tracker_writer.seek(SeekFrom::Start(0));
        write!(tracker_writer, "{}", total_bytes_read);
        thread::spawn(move || orchestrator::orchestrate(transaction));
    }
    println!("All transactions were processed");
}
