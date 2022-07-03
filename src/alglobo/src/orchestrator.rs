use std::net::UdpSocket;
use std::sync::Barrier;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use crate::Logger;

const HOTEL_ADDR: &str = "127.0.0.1:9000";
const BANK_ADDR: &str = "127.0.0.1:9001";
const AER_ADDR: &str = "127.0.0.1:9002";
const TTL: Duration = Duration::from_secs(2);
const MAX_SIMULATE_WORK: u64 = 2;

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

pub fn orchestrate(msg: String, mut logger: Logger) {
    let v: Vec<&str> = msg.trim().split(",").collect();
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
            //Write into deadletter
        }
        should_continue = *f;
    }
    if should_continue {
        barrier.wait(); // commit
    } else {
        logger.log(format!("[{}] i failed", id).as_str(), "ERROR");
    }
    for t in v {
        t.join().expect("will not fail");
    } //Ending method
    //write in logger
}
