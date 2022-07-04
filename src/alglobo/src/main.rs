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

    let deadletter = File::options().append(false).read(true).write(true).create(true).open(DEADLETTER_FILE);
    let mut deadletter_reader;
    let deadletter_c;
    if let Ok(ref file) = deadletter {
        deadletter_reader = BufReader::new(file);
        deadletter_c  = file.try_clone().unwrap();
    }else {
        println!("Error opening {}", DEADLETTER_FILE);
        return;
    }

    let mut deadletter_writer = BufWriter::new(&deadletter_c);
    let mut seek = 0;

    let mut line = "".to_string();
    let mut dead_transaction;
    let mut state;

    let mut input_string = String::new();
    let mut skipped = false;

    let mut read_bytes = deadletter_reader.read_line(&mut line).unwrap();
    while read_bytes > 0 && !skipped {

        state = line[read_bytes-2..read_bytes-1].to_string();
        dead_transaction = line[0..read_bytes-3].to_string();

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
                        orchestrator::orchestrate((&dead_transaction).to_string());
    
                        deadletter_writer.seek(SeekFrom::Start(seek));
                        write!(deadletter_writer,"{},P\n",dead_transaction);
                        break;
                    },
                    "n" => {
                        println!("Ignoring transaction..");
                        break;
                    },
                    "r" => {
                        println!("Removing transaction..");
                        deadletter_writer.seek(SeekFrom::Start(seek));
                        write!(deadletter_writer,"{},R\n",dead_transaction);
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
        read_bytes = deadletter_reader.read_line(&mut line).unwrap();
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

    //let mock_msg = "some,0,20,0".to_owned();
    //orchestrator::orchestrate(mock_msg);
    let mut threads = vec![];

    let tracker_file = File::options().append(false).read(true).write(true).create(true).open(TRACKER);
    let tracker_reader;
    let tf;
    if let Ok(ref file) = tracker_file {
        tracker_reader = BufReader::new(file);
        tf = file.try_clone().unwrap();
    } else {
        println!("Error with tracker file");
        return;
    }
    let mut tracker_writer = BufWriter::new(&tf);
    let pointer = 0;//get_start_position(tracker_reader);

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
        let t = thread::spawn(move || orchestrator::orchestrate(transaction.trim_end().to_string().to_owned()));
        threads.push(t);
    }

    for t in threads{
        t.join();
    }
    println!("All transactions were processed");

}
