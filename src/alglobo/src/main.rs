mod orchestrator;

use std::net::UdpSocket;
use std::process::exit;
use std::sync::Barrier;
use std::sync::{Arc, RwLock};
use std::{io, thread};
use std::thread::JoinHandle;
use std::time::Duration;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write, stdin};
use std::env;

fn id_to_ctrladdr(id: usize) -> String { "127.0.0.1:1234".to_owned() + &*id.to_string() }
fn id_to_dataaddr(id: usize) -> String { "127.0.0.1:1235".to_owned() + &*id.to_string() }
const ADDR: &str = "127.0.0.1:8000";
const TRACKER: &str = "tracker";
const TRANSACTIONS_FILE: &str = "transactions";
const DEADLETTER_FILE: &str = "dead_letter";
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


// 1. hay que limpiar las dead_transacctions del archivo cuando se completen. Como validar que se procesaron correctamente?
// 2. habría que cuidar el acceso concurrente al archivo desde los otros threads
// 3. En el caso que decidamos no limpiar las transacciones (1.) entonces podemos armar un sistema de punteros igual que para la lectura de la cola de transacciones .
fn read_dead_letter() {

    let deadletter = File::open(DEADLETTER_FILE);
    if let Err(error) = deadletter {
        println!("Error opening {} {}", DEADLETTER_FILE, error);
        return;
    }

    let mut deadletter_reader = BufReader::new(deadletter.unwrap());
    let mut dead_transaction = "".to_string();

    let mut input_string = String::new();
    let mut skipped = false;

    let mut read_bytes = deadletter_reader.read_line(&mut dead_transaction).unwrap();
    while read_bytes > 0 && !skipped {

        print!("Transacción => {} | ",&dead_transaction.trim_end());
        print!("Select an option: [P]roccess - [I]gnore - [S]kip: ");
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
                    orchestrator::orchestrate((&dead_transaction.trim_end()).to_string());
                    break;
                },
                "i" => {
                    println!("Ignoring transaction..");
                    break;
                },
                "e" => {
                    println!("Exiting the process of deadletter..");
                    skipped = true;
                    break;
                },
                &_ => {
                    println!("{} is not valid. Only P/I/S keys are valid.. ",(&input_string.to_lowercase()).as_str().trim_end() );
                }
            }
        }
  
        dead_transaction.clear();
        read_bytes = deadletter_reader.read_line(&mut dead_transaction).unwrap();
    }
}

fn main() {
 
    // Arguments format:  script_name [--deadletter | -D]
    
    let args: Vec<String> = env::args().collect();
    for arg in args{
        if arg == "--deadletter" || arg == "-D" {
            read_dead_letter();
            return;
        }
    }

    let mock_msg = "some,0,20,0".to_owned();
    orchestrator::orchestrate(mock_msg);

    let mut tracker_file = File::options().append(false).read(true).write(true).create(true).open(TRACKER);
    let mut tracker_reader;
    let tf;
    if let Ok(ref file) = tracker_file {
        tracker_reader = BufReader::new(file);
        tf = file.try_clone().unwrap();
    } else {
        println!("Error with tracker file");
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
        println!("{}",&transaction);
        thread::spawn(move || orchestrator::orchestrate(transaction));
    }
    println!("All transactions were processed");

}
