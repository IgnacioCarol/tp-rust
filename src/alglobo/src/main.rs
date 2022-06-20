use std::net::UdpSocket;
use std::sync::Barrier;
use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

fn id_to_ctrladdr(id: usize) -> String { "127.0.0.1:1234".to_owned() + &*id.to_string() }
fn id_to_dataaddr(id: usize) -> String { "127.0.0.1:1235".to_owned() + &*id.to_string() }
const ADDR: &str = "127.0.0.1:8000";
const HOTEL_ADDR: &str = "127.0.0.1:9000";
const BANK_ADDR: &str = "127.0.0.1:9001";
const AER_ADDR: &str = "127.0.0.1:9002";
const TTL: Duration = Duration::from_secs(2);

struct AlGlobo {
    id: String,
    ctrl_socket: UdpSocket
//Add extra data required for election algorithm
}

fn send_req(addr: String, amount: i64, barrier: Arc<Barrier>, flag: Arc<RwLock<bool>>, id: String) {
    barrier.wait();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap(); // With 0 port, OS will give us one
    let prepare = format!("P {} {}", id, amount);
    let mut continue_sending = true;
    socket.set_read_timeout(Some(TTL));
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

fn orchestrator(msg: String) {
    let v: Vec<&str> = msg.split(",").collect();
    let (id, amount_bank, amount_hotel, amount_aer) = (v[0].to_owned(), v[1].parse::<i64>().unwrap(), v[2].parse::<i64>().unwrap(), v[3].parse::<i64>().unwrap());
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
        println!("sending to hotel");
        let b = barrier.clone();
        let f = flag.clone();
        let i = id.clone();
        v.push(thread::spawn(move || send_req(HOTEL_ADDR.to_owned(), amount_hotel, b, f, i)));
    }
    if amount_bank != 0 {
        println!("sending to bank");
        let b = barrier.clone();
        let f = flag.clone();
        let i = id.clone();
        v.push(thread::spawn(move || send_req(BANK_ADDR.to_owned(), amount_bank, b, f, i)));
    }
    if amount_aer != 0 {
        let b = barrier.clone();
        let f = flag.clone();
        let i = id.clone();
        v.push(thread::spawn(move || send_req(AER_ADDR.to_owned(), amount_aer, b, f, i)));
    }
    barrier.wait();
    barrier.wait();
    let mut should_continue = false;
    if let Ok(f) = flag.read() {
        if !*f {
            //Write into deadletter
        }
        should_continue = *f;
    }
    if should_continue {
        barrier.wait();
    }
    for t in v {
        t.join();
    } //Ending method
    //write in logger
}

impl AlGlobo {

}

fn main() {
    let mock_msg = "some,0,20,0".to_owned();
    orchestrator(mock_msg);
}
