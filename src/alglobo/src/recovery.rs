use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};

use crate::dead_letter::new_dead;
use crate::file_handler::FileHandler;

const RECOVERY_FILE: &str = "recovery.txt";

pub fn mark_transaction(transaction: &str, flag: &str) {
    
    let result = FileHandler::new(RECOVERY_FILE.to_string()) ;
    if let Err(e) = result {
        println!("Error");
        return;
    }
    let mut recovery_file = result.unwrap();

    let target: Vec<&str> = transaction.split(',').collect();
    let id_target = target[0].to_owned();

    let mut seek = 0;

    while let Some(line) = recovery_file.read() {

        let splitted: Vec<&str> = line.trim_end().split(',').collect();
        let id = splitted[0].to_owned();

        seek += line.len() as u64;

        if id == id_target {
            recovery_file.seek(seek - 3);
            recovery_file.write(flag);
            return;
        }
    }

    recovery_file.seek(seek);
    recovery_file.write(&format!("{},{}\n", transaction, flag));

}

pub fn start_recovery() {

    let result = FileHandler::new(RECOVERY_FILE.to_string()) ;
    if let Err(e) = result {
        println!("Error");
        return;
    }
    let mut recovery_file = result.unwrap();

    let mut status: String;

    while let Some(line) = recovery_file.read() {

        let splt: Vec<&str> = line.trim_end().split(',').collect();
        status = splt[4].to_owned();

        if status == "--" {
            new_dead(line[0..line.len() - 4].to_string());
        }
    }

    fs::remove_file(RECOVERY_FILE).unwrap();
}
