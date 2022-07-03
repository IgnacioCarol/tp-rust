use std::fs;
use std::io::Write;
use std::sync::Arc;
use chrono::Local;
use std_semaphore::Semaphore;

const PATH: &str = "logger.txt";
const PATH_DEADLETTER: &str = "cosito.txt";

pub struct Logger {
    path: String,
    sem: Arc<Semaphore>,
}

impl Logger {
    pub fn new() -> Logger {
        let sem = Arc::new(Semaphore::new(1));
        Logger {
            path: PATH.to_string(),
            sem,
        }
    }

    pub fn change_leader(&mut self) {
        self.path = format!("leader-{}.txt", PATH);
    }

    pub fn change_id_path(&mut self, id: usize) {
        self.path = format!("{}-{}.txt", id, PATH);
    }

    pub fn clone(&self) -> Logger {
        let sem = self.sem.clone();
        Logger {
            path: self.path.clone(),
            sem,
        }
    }

    pub fn log(&mut self, msg: &str, status: &str) {
        self.sem.acquire();
        let mut file = fs::OpenOptions::new().write(true).append(true).create(true).open(self.path.as_str()).unwrap();
        let date = Local::now();
        let msg = format!("{} || {}=> {}\n", date.format("%Y-%m-%d - %H:%M:%S"), status, msg);
        file.write(msg.as_bytes()).expect("could not use logger");
        self.sem.release();
    }
    pub fn log_info(&mut self, msg: String) {
        self.log(msg.as_str(), "INFO");
    }

    pub fn log_into_deadletter(&mut self, msg: &str) {
        self.sem.acquire();
        let mut file = fs::OpenOptions::new().write(true).append(true).create(true).open(PATH_DEADLETTER).unwrap();
        file.write(msg.as_bytes()).expect("could not use logger");
        self.sem.release();
    }
}
