
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};

struct DeadLetter {
    path : String
}

impl DeadLetter {

    fn new(path:String) -> DeadLetter {
        return DeadLetter{ path };
    }

    fn read_dead_letter(&self) {

        // Funcion procesar que esta en main
    }

    fn new_dead(transaction: String){

        // Funcion new_dead que esta en el orchestrator
    }
}