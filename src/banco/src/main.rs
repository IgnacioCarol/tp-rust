extern crate actix;
extern crate chrono;

use actix::{Actor, AsyncContext, Context, Handler, Message, Recipient};
use chrono::Local;
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::str;

const ADDR: &str = "127.0.0.1:9001";
const STATUS_INFO: &str = "INFO";
const STATUS_ERROR: &str = "ERROR";
const PATH: &str = "./banco.txt";
const CHANCE_TO_ABORT: usize = 10;

#[derive(Message)]
#[rtype(result = "i64")]
struct Add(i64);

struct Prepare(String, SocketAddr);

impl Message for Prepare {
    type Result = ();
}
struct Commit(String, SocketAddr);

impl Message for Commit {
    type Result = ();
}

struct Abort(String, SocketAddr);

impl Message for Abort {
    type Result = ();
}

struct Process(String, SocketAddr);

impl Message for Process {
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

impl Handler<Log> for Logger {
    type Result = ();

    fn handle(&mut self, msg: Log, _ctx: &mut Self::Context) -> Self::Result {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(PATH)
            .unwrap();
        let date = Local::now();
        let msg = format!(
            "{} || {}=> {}\n",
            date.format("%Y-%m-%d - %H:%M:%S"),
            msg.0.1,
            msg.0.0
        );
        let _ = file.write(msg.as_bytes()).unwrap();
    }
}

struct Banco {
    amount: i64,
    logger: Recipient<Log>,
    transaction_logger: HashMap<String, (String, i64)>,
    socket: UdpSocket,
}

impl Actor for Banco {
    type Context = Context<Self>;
}

impl Handler<Add> for Banco {
    type Result = i64;
    fn handle(&mut self, msg: Add, _ctx: &mut Self::Context) -> Self::Result {
        self.amount += msg.0;
        self.logger
            .do_send(Log((
                format!("[BANCO ACTOR] Amount changed, current is: {}", self.amount),
                STATUS_INFO.to_string(),
            )))
            .expect("should be sent");
        self.amount
    }
}

impl Handler<Commit> for Banco {
    type Result = ();
    fn handle(&mut self, msg: Commit, _ctx: &mut Self::Context) -> Self::Result {
        let information = msg.0;
        self.logger.do_send(Log((format!("being committed with id {}", information), STATUS_INFO.to_string()))).unwrap();
        let mut result = "ok";
        let amount = self.transaction_logger.get(information.as_str()).cloned();
        match amount {
            None => {
                result = "fl";
            }
            Some(v) => {
                if v.0 == "P" {
                    self.transaction_logger.insert(information.to_string(), ("C".to_string(), v.1));
                } else {
                    self.logger.do_send(Log((
                        format!(
                            "message had the following status: {}, not being commited",
                            v.0
                        ),
                        STATUS_INFO.to_string()))
                    ).unwrap();
                }
            }
        }
        if result == "fl" {
            self.logger.do_send(Log((
                format!("id {} did not exist", information),
                STATUS_ERROR.to_string(),
            ))).unwrap();
        }
        self.socket
            .send_to(result.as_bytes(), msg.1)
            .expect("socket broken");
    }
}


impl Handler<Prepare> for Banco {
    type Result = ();

    fn handle(&mut self, msg: Prepare, _ctx: &mut Self::Context) -> Self::Result {
        let address = msg.1;
        let information = msg.0.as_str();
        let v: Vec<&str> = information.split(' ').collect();
        let (id, amount) = (v[0], v[1].parse::<i64>().unwrap());

        let mut success = true;
        let value = self.transaction_logger.get(id).cloned();
        match value {
            None => {
                if thread_rng().gen_range(0, 100) >= 100 - CHANCE_TO_ABORT {
                    self.logger.do_send(Log((
                        format!("failing transaction {}", id),
                        STATUS_INFO.to_string(),
                    ))).unwrap();
                    self.transaction_logger.insert(id.to_string(), ("C".to_string(), amount));
                    success = false;
                } else {
                    self.transaction_logger.insert(id.to_string(), ("P".to_string(), amount));
                }
            }
            Some(v) => {
                if v.0 == "A" {
                    self.transaction_logger.insert(id.to_string(), ("P".to_string(), v.1));
                }
            }
        }
        if success {
            self.logger.do_send(Log((
                format!("preparing with id {} and amount {}", id, amount),
                STATUS_INFO.to_string(),
            ))).unwrap();
            self.socket
                .send_to("ok".as_bytes(), address)
                .expect("socket broken");
        } else {
            self.logger.do_send(Log((format!("aborting with id {}", id), STATUS_ERROR.to_string()))).unwrap();
            self.socket
                .send_to("fl".as_bytes(), address)
                .expect("socket broken");
        }
    }
}

impl Handler<Abort> for Banco {
    type Result = ();
    fn handle(&mut self, msg: Abort, _ctx: &mut Self::Context) -> Self::Result {
        let information = msg.0.as_str();
        let mut was_added = true;
        let value = self.transaction_logger.get(information).cloned();
        match value {
            None => {
                was_added = false;
            }
            Some(v) => {
                if v.0 == "A" {
                    self.transaction_logger.insert(information.to_string(), ("A".to_string(), v.1));
                }
            }
        }
        if !was_added {
            self.logger.do_send(Log((
                format!("transaction {} never was added", information),
                STATUS_ERROR.to_string(),
            ))).unwrap();
            return;
        }
        self.logger.do_send(Log((
            format!("aborting transaction {}", information),
            STATUS_INFO.to_string(),
        ))).unwrap();
        self.socket
            .send_to("ok".as_bytes(), msg.1)
            .expect("socket broken");
    }
}

impl Handler<Process> for Banco {
    type Result = ();
    fn handle(&mut self, msg: Process, ctx: &mut Self::Context) -> Self::Result {
        let message = msg.0;
        let address = msg.1;
        self.logger.do_send(Log((
            format!("message received from {} is {}", address, message),
            STATUS_INFO.to_string(),
        ))).unwrap();
        let (intention, mut information) = message.split_at(1);
        information = information.trim();
        match intention {
            "C" => { ctx.notify(Commit(information.to_string(), address)) }
            "P" => {
                ctx.notify(Prepare(information.to_string(), address))
            }
            "A" => {ctx.notify(Abort(information.to_string(), address))
            }
            &_ => {
                self.logger.do_send(Log((
                    format!("intention {} not recognized", intention),
                    STATUS_ERROR.to_string(),
                ))).unwrap();
                self.socket
                    .send_to("fl".as_bytes(), address)
                    .expect("socket broken");
            }
        }
    }
}

#[actix_rt::main]
async fn main() {
    let logger = Logger {}.start();
    let l_1 = logger.clone().recipient();
    let actor_banco = Banco {
        amount: 0,
        logger: l_1,
        socket: UdpSocket::bind("127.0.0.1:0").unwrap(),
        transaction_logger: HashMap::new()
    }.start();
    logger
        .try_send(Log(("socket started".to_string(), STATUS_INFO.to_string())))
        .unwrap();
    let socket = actix_rt::net::UdpSocket::bind(ADDR).await.unwrap();
    loop {
        let mut buf = [0; 1024];
        let (size, from) = socket.recv_from(&mut buf).await.unwrap();
        let msg = str::from_utf8(&buf[0..size]).unwrap().to_owned().clone();
        actor_banco.try_send(Process(msg, from)).unwrap();
    }
}
