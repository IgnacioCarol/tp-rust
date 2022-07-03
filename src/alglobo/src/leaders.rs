use std::mem::size_of;
use std::net::{UdpSocket};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use std::convert::TryInto;
use std::ops::Deref;
use std_semaphore::Semaphore;
use crate::logger::Logger;

fn id_to_ctrladdr(id: usize) -> String { "127.0.0.1:1234".to_owned() + &*id.to_string() }

fn id_to_dataaddr(id: usize) -> String { "127.0.0.1:1235".to_owned() + &*id.to_string() }

const TIMEOUT: Duration = Duration::from_secs(10);
const STATUS_INFO: &str = "INFO";
const STATUS_ERROR: &str = "ERROR";
const TIME_BETWEEN_CHECKS: u64 = 3;
//As seconds
const TEAM_MEMBERS: usize = 10;
//We will assume that there will never be more than 10 members
const READ_TIMEOUT_CHECK: u64 = 10;
const TTL_LEADER: Duration = Duration::from_secs(50);

pub struct LeaderElection {
    id: usize,
    socket: UdpSocket,
    leader_id: Arc<(Mutex<Option<usize>>, Condvar)>,
    got_ok: Arc<(Mutex<bool>, Condvar)>,
    stop: Arc<(Mutex<bool>, Condvar)>,
    logger: Logger,
    sem: Arc<Semaphore>,
    last_leader_id: Arc<RwLock<usize>>,
}

impl LeaderElection {
    fn new(id: usize, logger: Logger) -> LeaderElection {
        let mut ret = LeaderElection {
            id,
            socket: UdpSocket::bind(id_to_ctrladdr(id)).unwrap(),
            leader_id: Arc::new((Mutex::new(Some(id)), Condvar::new())),
            got_ok: Arc::new((Mutex::new(false), Condvar::new())),
            stop: Arc::new((Mutex::new(false), Condvar::new())),
            logger,
            sem: Arc::new(Semaphore::new(0)),
            last_leader_id: Arc::new(RwLock::new(100)),
        };

        let mut clone = ret.clone();
        thread::spawn(move || clone.responder());

        ret.find_new();
        ret
    }

    fn am_i_leader(&self) -> bool {
        self.get_leader_id() == self.id
    }

    pub fn leader(&self) -> bool {
        let mut li = 100;
        if let Ok(leader_id) = self.last_leader_id.read() {
            li = *leader_id;
        }
        return li == self.id;
    }

    pub fn wake_me_up_when_september_ends(&self) {
        self.sem.acquire();
    }

    fn get_leader_id(&self) -> usize {
        if let Ok(data) = self.leader_id.0.lock() {
            match data.deref() {
                Some(number) => return number.clone(),
                _ => 1 + 1,
            };
        }
        self.leader_id.1.wait_while(self.leader_id.0.lock().unwrap(), |leader_id| leader_id.is_none()).unwrap().unwrap()
    }

    fn find_new(&mut self) {
        if *self.stop.0.lock().unwrap() {
            return;
        }
        if self.leader_id.0.lock().unwrap().is_none() {
            // ya esta buscando lider
            return;
        }
        self.logger.log("looking for leader", STATUS_INFO);
        *self.got_ok.0.lock().unwrap() = false;
        *self.leader_id.0.lock().unwrap() = None;
        self.logger.change_id_path(self.id);
        self.send_election();
        let got_ok = self.got_ok.1.wait_timeout_while(self.got_ok.0.lock().unwrap(), TTL_LEADER, |got_it| !*got_it);
        if !*got_ok.unwrap().0 {
            self.make_me_leader()
        } else {
            self.leader_id.1.wait_while(self.leader_id.0.lock().unwrap(), |leader_id| leader_id.is_none()).unwrap();
        }
    }

    fn id_to_msg(&self, header: u8) -> Vec<u8> {
        let mut msg = vec!(header);
        msg.extend_from_slice(&self.id.to_le_bytes());
        msg
    }

    fn send_election(&self) {
        // P envía el mensaje ELECTION a todos los procesos que tengan número mayor
        let msg = self.id_to_msg(b'E');
        for peer_id in (self.id + 1)..TEAM_MEMBERS {
            self.socket.send_to(&msg, id_to_ctrladdr(peer_id)).unwrap();
        }
    }

    fn make_me_leader(&mut self) {
        // El nuevo coordinador se anuncia con un mensaje COORDINATOR
        self.logger.log("making me leader", STATUS_INFO);
        self.logger.change_leader();
        self.logger.log(format!("[{}] I am your new master", self.id).as_str(), STATUS_INFO);
        let msg = self.id_to_msg(b'C');
        for peer_id in 0..TEAM_MEMBERS {
            if peer_id != self.id {
                self.socket.send_to(&msg, id_to_ctrladdr(peer_id)).unwrap();
            }
        }
        *self.leader_id.0.lock().unwrap() = Some(self.id);
        if let Ok(mut last_id) = self.last_leader_id.write() {
            *last_id = self.id;
        }
        self.sem.release();
        self.leader_id.1.notify_all();
    }

    fn step_down(&mut self) {
        if let Ok(mut last_id) = self.last_leader_id.write() {
            *last_id = 100;
        }
    }
    fn responder(&mut self) {
        while !*self.stop.0.lock().unwrap() {
            let mut buf = [0; size_of::<usize>() + 1];
            let (_size, from) = self.socket.recv_from(&mut buf).unwrap();
            let id_from = usize::from_le_bytes(buf[1..].try_into().unwrap());
            if *self.stop.0.lock().unwrap() {
                break;
            }
            match &buf[0] {
                b'O' => {
                    self.logger.log(format!("Received ok from {}", id_from).as_str(), STATUS_INFO);
                    *self.got_ok.0.lock().unwrap() = true;
                    if let Ok(mut leader_id) = self.last_leader_id.write() {
                        *leader_id = 100;
                    }
                    self.got_ok.1.notify_all();
                }
                b'E' => {
                    self.logger.change_id_path(self.id);
                    self.logger.log(format!("received election from {}", id_from).as_str(), STATUS_INFO);
                    if id_from < self.id {
                        self.socket.send_to(&self.id_to_msg(b'O'), id_to_ctrladdr(id_from)).unwrap();
                        let mut me = self.clone();
                        thread::spawn(move || me.find_new());
                    }
                }
                b'C' => {
                    self.logger.log(format!("Received new coordinator from {}", id_from).as_str(), STATUS_INFO);
                    if id_from < self.id {
                        self.socket.send_to(&self.id_to_msg(b'O'), id_to_ctrladdr(id_from)).unwrap();
                    } else {
                        *self.leader_id.0.lock().unwrap() = Some(id_from);
                        self.leader_id.1.notify_all();
                        self.step_down();
                    }
                }
                _ => {
                    self.logger.log(format!("received weird stuff from {}, starting message {}", from, id_from).as_str(), STATUS_ERROR);
                }
            }
        }
        *self.stop.0.lock().unwrap() = false;
        self.stop.1.notify_all();
    }

    fn clone(&self) -> LeaderElection {
        LeaderElection {
            id: self.id,
            socket: self.socket.try_clone().unwrap(),
            leader_id: self.leader_id.clone(),
            got_ok: self.got_ok.clone(),
            stop: self.stop.clone(),
            logger: self.logger.clone(),
            sem: self.sem.clone(),
            last_leader_id: self.last_leader_id.clone(),
        }
    }
}

pub fn get_leader_election(id: usize, mut logger: Logger) -> LeaderElection {
    logger.change_id_path(id);
    let log = logger.clone();
    let le = LeaderElection::new(id, logger);
    let copy_le = le.clone();
    thread::spawn(move || team_member(copy_le, log));
    return le;
}

fn team_member(mut leader: LeaderElection, mut logger: Logger) {
    let id = leader.id;
    let socket = UdpSocket::bind(id_to_dataaddr(id)).unwrap();
    let mut buf = [0; 5];

    loop {
        if leader.am_i_leader() {
            logger.log("I am the leader", STATUS_INFO);
            socket.set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_CHECK))).unwrap();
            if let Ok((_size, from)) = socket.recv_from(&mut buf) {
                socket.send_to("Beat".as_bytes(), from).unwrap();
            } else {
                logger.log_info("no one wants me".to_string());
            };
        } else {
            let leader_id = leader.get_leader_id();
            logger.log("starting heartbeat", STATUS_INFO);
            socket.send_to("Heart".as_bytes(), id_to_dataaddr(leader_id)).unwrap();
            socket.set_read_timeout(Some(TIMEOUT)).unwrap();
            if let Ok((_size, _from)) = socket.recv_from(&mut buf) {
                logger.log("heartbeat completed", STATUS_INFO);
                thread::sleep(Duration::from_secs(TIME_BETWEEN_CHECKS));
            } else {
                logger.log("something happened to leader, checking election", STATUS_ERROR);
                leader.find_new()
            }
        }
    }
}
