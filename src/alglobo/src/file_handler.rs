
use std::fs::{File};
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};


pub struct FileHandler {
    reader: BufReader<File>,
    writer: BufWriter<File>, 
    seek : u64,
    bytes_read : usize
}

impl FileHandler {


//     let tracker_file = File::options()
//     .append(false)
//     .read(true)
//     .write(true)
//     .create(true)
//     .open(TRACKER);
// let tracker_reader;
// let tf;
// if let Ok(ref file) = tracker_file {
//     tracker_reader = BufReader::new(file);
//     tf = file.try_clone().unwrap();
// } else {
//     logger.log("Error with tracker file", STATUS_ERROR);
//     return;
// }
// let mut tracker_writer = BufWriter::new(&tf);



    pub fn new( path : String) -> Result<FileHandler, std::io::Error> {

        let file = File::options()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(path);

        if let Err( e) = file {
            return Err(e);
        }

        let copy = file.unwrap().try_clone().unwrap();
        let reader = BufReader::new(copy.try_clone().unwrap());
        let writer = BufWriter::new(copy);

        return Ok(  FileHandler{ reader, writer, seek: 0, bytes_read: 0 } );

      
    }

    pub fn read(&mut self) -> Option<String> {

        let mut line = String::from("");

        if let Ok(bytes_read) = self.reader.read_line(&mut line){
  
            self.bytes_read += bytes_read;

            if bytes_read != 0 {
                return Some(line);
            }
        }

        return None;
    }

    pub fn write(&mut self, content : &str){
     
        //self.writer.seek(SeekFrom::Start(self.seek));
        write!(self.writer,"{}", content);
        self.writer.flush();
        //self.seek += content.len() as u64;
    }

    pub fn seek(&mut self, pos: u64){
        self.writer.seek(SeekFrom::Start(pos));
        self.seek = pos;
    }

}

