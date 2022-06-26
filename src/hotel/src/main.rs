extern crate actix;
extern crate chrono;

use chrono::Local;
use std::collections::{HashMap};
use std::net::{SocketAddr, UdpSocket};
use std::str;
use std::sync::{Arc, RwLock};
use std::thread;
use std::fs;
use std::io::{Write};
use std::time;
use std::time::SystemTime;
use actix::{Actor, Addr, Context, Handler, Message};
use std_semaphore::Semaphore;

const ADDR: &str = "127.0.0.1:9000";
const STATUS_INFO: &str = "INFO";
const STATUS_ERROR: &str = "ERROR";
const PATH: &str = "./hotel.txt";

#[derive(Message)]
#[rtype(result = "i64")]
struct Add(i64);

#[derive(Message)]
#[rtype(result = "i64")]
struct Sub(i64);

struct Hotel {
    amount: i64,
}

impl Actor for Hotel {
    type Context = Context<Self>;
}

impl Handler<Add> for Hotel {
    type Result = i64;
    fn handle(&mut self, msg: Add, _ctx: &mut Self::Context) -> Self::Result {
        self.amount += msg.0;
        self.amount
    }
}

impl Handler<Sub> for Hotel {
    type Result = i64;
    fn handle(&mut self, msg: Sub, _ctx: &mut Self::Context) -> Self::Result {
        self.amount -= msg.0;
        self.amount
    }
}

struct HotelSocket {
    socket: UdpSocket,
    transaction_logger: Arc<RwLock<HashMap<String, (String, i64)>>>,
    actor: Arc<Addr<Hotel>>,
    logger: Arc<Semaphore>,
    starting_time: SystemTime,
}

impl HotelSocket {
    fn new(hotel_actor: Addr<Hotel>) -> HotelSocket {
        HotelSocket {
            socket: UdpSocket::bind(ADDR).unwrap(),
            transaction_logger: Arc::new(RwLock::new(HashMap::new())),
            actor: Arc::new(hotel_actor),
            logger: Arc::new(Semaphore::new(1)),
            starting_time: time::SystemTime::now(),
        }
    }

    fn clone(&self) -> HotelSocket {
        HotelSocket {
            socket: self.socket.try_clone().unwrap(),
            transaction_logger: self.transaction_logger.clone(),
            actor: self.actor.clone(),
            logger: self.logger.clone(),
            starting_time: self.starting_time.clone(),
        }
    }
    fn responder(&mut self) {
        loop {
            let mut buf = [0; 1024];
            let (size, from) = self.socket.recv_from(&mut buf).unwrap();
            let mut c = self.clone();
            let msg = str::from_utf8(&buf[0..size]).unwrap().to_owned().clone();
            thread::spawn(move || c.process_message(msg, from));
        }
    }

    fn process_message(&mut self, msg: String, address: SocketAddr) {
        self.write_into_logger(&format!("message received from {} is {}", address, msg), STATUS_INFO);
        let (intention, mut information) = msg.split_at(1);
        information = information.trim();
        match intention {
            "C" => {
                self.write_into_logger(&format!("being committed with id {}", information), STATUS_INFO);
                let mut result = "ok";
                if let Ok(mut data) = self.transaction_logger.write() {
                    let amount = data.get(information).cloned();
                    match amount {
                        None => {
                            result = "fl";
                        }
                        Some(v) => {
                            if v.0 == "P" {
                                data.insert(information.to_string(),("C".to_string(), v.1));
                            }
                            self.actor.do_send(Add(v.1))
                        }
                    }
                }
                if result == "fl" {
                    self.write_into_logger(&format!("id {} did not exist", information), STATUS_ERROR);
                }
                self.socket.send_to(result.as_bytes(), address).expect("socket broken");
            } // commit
            "P" => {
                let v: Vec<&str> = information.split(" ").collect();
                let (id, amount) = (v[0], v[1].parse::<i64>().unwrap());
                if let Ok(mut data) = self.transaction_logger.write() {
                    let value = data.get(id).cloned();
                    match value {
                        None => {
                            data.insert(id.to_string(), ("P".to_string(), amount));
                        }
                        Some(v) => {
                            if v.0 == "A" {
                                data.insert(id.to_string(), ("P".to_string(), v.1));
                            }
                        }
                    }
                }
                self.write_into_logger(&format!("preparing with id {} and amount {}", id, amount), STATUS_INFO);
                self.socket.send_to("ok".as_bytes(), address).expect("socket broken");
            }
            "A" => {
                let mut was_added = true;
                if let Ok(mut data) = self.transaction_logger.write() {
                    let value = data.get(information).cloned();
                    match value {
                        None => {
                            was_added = false;
                        }
                        Some(v) => {
                            if v.0 == "A" {
                                data.insert(information.to_string(), ("A".to_string(), v.1));
                            }
                        }
                    }
                }
                if !was_added {
                    self.write_into_logger(&format!("transaction {} never was added", information), STATUS_ERROR);
                }
                self.write_into_logger(&format!("aborting transaction {}", information), STATUS_INFO);
                self.socket.send_to("ok".as_bytes(), address).expect("socket broken");
            }
            &_ => {
                self.write_into_logger(&format!("intention {} not recognized", intention), STATUS_ERROR);
                self.socket.send_to("fl".as_bytes(), address).expect("socket broken");
            }
        }
    }

    fn write_into_logger(&mut self, data: &str, status: &str) {
        self.logger.acquire();
        let mut file = fs::OpenOptions::new().write(true).append(true).create(true).open(PATH).unwrap();
        let date = Local::now();
        let msg = format!("{} || {}=>{}\n", date.format("%Y-%m-%d - %H:%M:%S"), status, data);
        file.write(msg.as_bytes()).expect("could not use logger");
        self.logger.release();
    }
}

#[actix_rt::main]
async fn main() {
    let actor_hotel = Hotel{amount: 0}.start();
    let a_h = actor_hotel.clone();
    let mut h = HotelSocket::new(a_h);
    h.write_into_logger("socket started", STATUS_INFO);
    h.responder();
}
