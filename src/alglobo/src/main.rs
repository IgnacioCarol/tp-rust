mod orchestrator;

use std::net::UdpSocket;
use std::sync::Barrier;
use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

fn id_to_ctrladdr(id: usize) -> String { "127.0.0.1:1234".to_owned() + &*id.to_string() }
fn id_to_dataaddr(id: usize) -> String { "127.0.0.1:1235".to_owned() + &*id.to_string() }
const ADDR: &str = "127.0.0.1:8000";
const TTL: Duration = Duration::from_secs(2);

struct AlGlobo {
    id: String,
    ctrl_socket: UdpSocket
//Add extra data required for election algorithm
}

impl AlGlobo {

}

fn main() {
    let mock_msg = "some,0,20,0".to_owned();
    orchestrator::orchestrate(mock_msg);
}
