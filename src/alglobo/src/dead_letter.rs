

use std::{io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write, stdin, self}, sync::{Arc, RwLock}, fs::File};
use std_semaphore::Semaphore;

use crate::{orchestrator, logger::Logger};

const DEADLETTER_FILE: &str = "dead_letter.txt";

pub(crate) fn new_dead(transaction: String){

    let deadletter =  File::options().append(false).read(true).write(true).create(true).open(DEADLETTER_FILE);
    if let Err(error) = deadletter {
        println!("Error opening {} {}", DEADLETTER_FILE, error);
        return;
    }

    let mut deadletter_writer = BufWriter::new(deadletter.unwrap());

    let _ = deadletter_writer.seek(SeekFrom::End(0));
    write!(deadletter_writer,"{},F\n",transaction).expect("Error al grabar dead transaction");
}


pub(crate) fn read_dead_letter() {
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