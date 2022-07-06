use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};

use crate::dead_letter::new_dead;

const RECOVERY_FILE: &str = "recovery.txt";

pub fn mark_transaction(transaction: &String, flag: &str) {
    let recovery_file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(RECOVERY_FILE);
    let mut recovery_reader;
    let recovery_c;
    if let Ok(ref file) = recovery_file {
        recovery_reader = BufReader::new(file);
        recovery_c = file.try_clone().unwrap();
    } else {
        //logger.log(format!("Error opening {}", RECOVERY_FILE).as_str(), "ERROR");
        return;
    }

    let target: Vec<&str> = transaction.split(",").collect();
    let id_target = target[0].to_owned();
    let mut line = String::new();

    let mut recovery_writer = BufWriter::new(&recovery_c);
    let mut seek = 0;

    let mut read_bytes = recovery_reader
        .read_line(&mut line)
        .expect("Cannot read file");
    while read_bytes > 0 {
        let splitted: Vec<&str> = line.trim_end().split(",").collect();
        let id = splitted[0].to_owned();

        seek += read_bytes as u64;

        if id == id_target {
            recovery_writer
                .seek(SeekFrom::Start(seek - 3))
                .expect("should move");
            write!(recovery_writer, "{}", flag).unwrap();
            recovery_writer.flush().unwrap();
            return;
        }

        line.clear();
        read_bytes = recovery_reader
            .read_line(&mut line)
            .expect("Cannot read file");
    }

    let _ = recovery_writer.seek(SeekFrom::End(0));
    write!(recovery_writer, "{},{}\n", transaction, flag)
        .expect("Error al grabar inicio de transaction");
    recovery_writer.flush().unwrap();
}

pub fn start_recovery() {
    let recovery_file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(RECOVERY_FILE);
    let mut recovery_reader;

    if let Err(_e) = recovery_file {
        // LOG error
        return;
    }

    recovery_reader = BufReader::new(recovery_file.unwrap());

    let mut line = String::new();
    let mut status: String;

    let mut read_bytes = recovery_reader
        .read_line(&mut line)
        .expect("Cannot read file");
    while read_bytes > 0 {
        let splt: Vec<&str> = line.trim_end().split(",").collect();
        status = splt[4].to_owned();

        if status == "--" {
            new_dead(line[0..read_bytes - 4].to_string());
        }

        line.clear();
        status.clear();
        read_bytes = recovery_reader
            .read_line(&mut line)
            .expect("Cannot read file");
    }

    fs::remove_file(RECOVERY_FILE).unwrap();
}
