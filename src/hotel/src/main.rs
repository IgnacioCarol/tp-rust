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
use actix::{Actor, Context, Handler, Message, Recipient};
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

struct Clear();
impl Message for Clear {
    type Result = ();
}

struct Log((String, String));

impl Message for Log {
    type Result = ();
}

struct Logger {}
impl Actor for Logger {
    type Context = Context<Self>;
}

impl Handler<Clear> for Logger {
    type Result = ();

    fn handle(&mut self, _msg: Clear, _ctx: &mut Self::Context) -> Self::Result {}
}

impl Handler<Clear> for Hotel {
    type Result = ();

    fn handle(&mut self, _msg: Clear, _ctx: &mut Self::Context) -> Self::Result {}
}

impl Handler<Log> for Logger {
    type Result = ();

    fn handle(&mut self, msg: Log, _ctx: &mut Self::Context) -> Self::Result {
        let mut file = fs::OpenOptions::new().write(true).append(true).create(true).open(PATH).unwrap();
        let date = Local::now();
        let msg = format!("{} || {}=> {}\n", date.format("%Y-%m-%d - %H:%M:%S"), msg.0.1, msg.0.0);
        file.write(msg.as_bytes()).expect("could not use logger");
    }
}

struct Hotel {
    amount: i64,
    logger: Recipient<Log>
}

impl Actor for Hotel {
    type Context = Context<Self>;
}

impl Handler<Add> for Hotel {
    type Result = i64;
    fn handle(&mut self, msg: Add, _ctx: &mut Self::Context) -> Self::Result {
        self.amount += msg.0;
        self.logger.do_send(Log((format!("[HOTEL ACTOR] Amount changed, current is: {}", self.amount), STATUS_INFO.to_string()))).expect("should be sent");
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
    actor: Arc<Recipient<Add>>,
    logger: Arc<Recipient<Log>>,
    message_sent: Arc<Semaphore>,
}

impl HotelSocket {
    fn new(hotel_actor: Recipient<Add>, logger_actor: Recipient<Log>, message_sent: Arc<Semaphore>) -> HotelSocket {
        HotelSocket {
            socket: UdpSocket::bind(ADDR).unwrap(),
            transaction_logger: Arc::new(RwLock::new(HashMap::new())),
            actor: Arc::new(hotel_actor),
            logger: Arc::new(logger_actor),
            message_sent,
        }
    }

    fn clone(&self) -> HotelSocket {
        HotelSocket {
            socket: self.socket.try_clone().unwrap(),
            transaction_logger: self.transaction_logger.clone(),
            actor: self.actor.clone(),
            logger: self.logger.clone(),
            message_sent: self.message_sent.clone(),
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
                                self.send_to_actor(v.1);
                            } else {
                                self.write_into_logger(format!("message had the following status: {}, not being commited", v.0).as_str(), STATUS_INFO);
                            }

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
        self.message_sent.release();
    }

    fn write_into_logger(&self, data: &str, status: &str) {
        self.logger.do_send(Log((data.to_string(), status.to_string()))).expect("should be ok");
    }

    fn send_to_actor(&self, amount_to_add: i64) {
        self.actor.do_send(Add(amount_to_add)).expect("should be ok");
    }
}

#[actix_rt::main]
async fn main() {
    let logger = Logger{}.start();
    let l_1 = logger.clone().recipient();
    let actor_hotel = Hotel{amount: 0, logger: l_1}.start();
    let a_h = actor_hotel.clone().recipient();
    let l_2 = logger.clone().recipient();
    let sem = Arc::new(Semaphore::new(0));
    let sc = sem.clone();
    let mut h = HotelSocket::new(a_h, l_2, sc);
    thread::spawn(move || h.responder());
    logger.send(Log(("socket started".to_string(), STATUS_INFO.to_string()))).await.unwrap();
    loop {
        sem.acquire();
        logger.send(Clear()).await.unwrap(); // Necessary to make the messages on the thread be processed
        actor_hotel.send(Clear()).await.unwrap();
    }
}
