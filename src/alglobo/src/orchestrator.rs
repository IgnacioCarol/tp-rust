use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use std::sync::Barrier;
use std::thread;
use std::time::Duration;

use std_semaphore::Semaphore;

use crate::recovery::mark_transaction;
use crate::{Logger, recovery};

const HOTEL_ADDR: &str = "127.0.0.1:9000";
const BANK_ADDR: &str = "127.0.0.1:9001";
const AER_ADDR: &str = "127.0.0.1:9002";
const TTL: Duration = Duration::from_secs(2);
const MAX_SIMULATE_WORK: u64 = 2;

const DEADLETTER_FILE: &str = "dead_letter";

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

fn send_req(addr: String, amount: i64, barrier: Arc<Barrier>, flag: Arc<RwLock<bool>>, id: String) {
    barrier.wait();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap(); // With 0 port, OS will give us one
    let prepare = format!("P {} {}", id, amount);
    let mut continue_sending = true;
    socket.set_read_timeout(Some(TTL)).expect("why would this fail?");
    let a = addr.clone();
    let _ = socket.send_to(prepare.as_bytes(), a).unwrap();
    let mut buf = [0; 1024];
    if let Ok(size) = socket.recv(buf.as_mut_slice()) {
        if &buf[0..size] != "ok".as_bytes() {
            continue_sending = false;
            if let Ok(mut f) = flag.write() {
                *f = continue_sending;
            }
        }
    } else {
        continue_sending = false;
        if let Ok(mut f) = flag.write() {
            *f = continue_sending;
        }
    }
    barrier.wait();
    if !continue_sending { // i finished
        return;
    }
    if let Ok(f) = flag.read() {
        if !*f {
            let abort = format!("A {}", id);
            let _ = socket.send_to(abort.as_bytes(), addr).unwrap();
            return;
        }
    }
    let commit = format!("C {}", id);
    let _ = socket.send_to(commit.as_bytes(), addr).unwrap();
    barrier.wait();
}

pub fn orchestrate(msg: String, mut logger: Logger, recover_sem : Arc<Semaphore>) {
    let v: Vec<&str> = msg.split(",").collect();
    if v.len() != 4 {
        logger.log(format!("Error in format for message {}", v[0]).as_str(), "ERROR");
        return
    }

    // Log start operation
    recover_sem.acquire();
    recovery::mark_transaction(&msg,"--");
    recover_sem.release();
    
    let (id, amount_air, amount_bank, amount_hotel) = (v[0].to_owned(), v[1].parse::<i64>().unwrap(), v[2].parse::<i64>().unwrap(), v[3].parse::<i64>().unwrap());
    let mut barrier_count = 0;
    for value in v {
        if value != "0" {
            barrier_count += 1;
        }
    };
    let barrier = Arc::new(Barrier::new(barrier_count));
    let flag = Arc::new(RwLock::new(true));
    let mut v = vec!();
    if amount_hotel != 0 {
        logger.log_info(format!("[{}] sending to hotel", id));
        let b = barrier.clone();
        let f = flag.clone();
        let i = id.clone();
        v.push(thread::spawn(move || send_req(HOTEL_ADDR.to_owned(), amount_hotel, b, f, i)));
    }
    if amount_bank != 0 {
        logger.log_info(format!("[{}] sending to bank", id));
        let b = barrier.clone();
        let f = flag.clone();
        let i = id.clone();
        v.push(thread::spawn(move || send_req(BANK_ADDR.to_owned(), amount_bank, b, f, i)));
    }
    if amount_air != 0 {
        logger.log_info(format!("[{}] sending to aer", id));
        let b = barrier.clone();
        let f = flag.clone();
        let i = id.clone();
        v.push(thread::spawn(move || send_req(AER_ADDR.to_owned(), amount_air, b, f, i)));
    }
    barrier.wait(); // To start all
    logger.log_info(format!("[{}] feels sleepy", id));
    thread::sleep(Duration::from_secs(MAX_SIMULATE_WORK));
    barrier.wait(); // Waiting until all finished preparing
    let mut should_continue = false;
    if let Ok(f) = flag.read() {
        if !*f {
            new_dead(msg.to_string());
            println!("{} has failed. Direct to the DEAD LETTER!!!",&id);

            recover_sem.acquire();
            mark_transaction(&msg,"DL");
            recover_sem.release();
        }else{
            println!("Todo ok");
        }
        should_continue = *f;
    }
    if should_continue {
        
        barrier.wait(); // commit

        recover_sem.acquire();
        mark_transaction(&msg,"OK");
        recover_sem.release();
    } else {
        logger.log(format!("[{}] i failed", id).as_str(), "ERROR");
    }
    for t in v {
        t.join().expect("will not fail");
    } //Ending method
    logger.log_info(format!("[{}] finished orchestrate", id));
}
