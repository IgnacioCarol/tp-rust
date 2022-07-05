
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use std::sync::Barrier;
use std::{thread, time};
use std::time::Duration;
use rand::Rng;
use std_semaphore::Semaphore;

use crate::dead_letter::new_dead;
use crate::recovery::mark_transaction;
use crate::{Logger, recovery};

const HOTEL_ADDR: &str = "127.0.0.1:9000";
const BANK_ADDR: &str = "127.0.0.1:9001";
const AER_ADDR: &str = "127.0.0.1:9002";
const TTL: Duration = Duration::from_secs(2);
const MAX_SIMULATE_WORK: u64 = 3000;

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

pub fn orchestrate(msg: String, mut logger: Logger, recover_sem : Arc<Semaphore>, time_avg : Arc<RwLock<(u64,u64)>>) {
    let v: Vec<&str> = msg.split(",").collect();
    if v.len() != 4 {
        logger.log(format!("Error in format for message {}", v[0]).as_str(), "ERROR");
        return
    }

    let start = time::Instant::now();

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
    let rand_work_time = rand::thread_rng().gen_range(1000,MAX_SIMULATE_WORK);
    thread::sleep(Duration::from_millis(rand_work_time));

    barrier.wait(); // Waiting until all finished preparing
    let mut should_continue = false;
    if let Ok(f) = flag.read() {
        if !*f {
            new_dead(msg.to_string());
            logger.log(&format!("Transaction {} FAILED. Sending to Deadletter",id),"ERROR");

            recover_sem.acquire();
            mark_transaction(&msg,"DL");
            recover_sem.release();
        }else{
            logger.log_info(format!("Transaction {} OK",id).to_string());
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
    
    logger.log_info(format!("[{}] finished orchestrate.", id));

    let elapsed = start.elapsed();
    
    if let Ok(mut a) = time_avg.write() {
        a.0 += 1;
        a.1 += elapsed.as_millis() as u64;
        let calc1 = a.1 as f64 /1000 as f64;
        let calc2 = calc1/((a.0) as f64);
        logger.log_info(format!("Total time: {:.2}s - Transactions: {} - Average Time: {:.2}s",calc1,a.0,calc2));        
    }    

}

