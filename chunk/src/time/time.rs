use std::{time::{SystemTime, UNIX_EPOCH}, io::{Error, BufReader, BufRead, ErrorKind}, fs::File};

use rsntp::{SntpClient};


const NTP_FILE_PATH:&str = "src/time/timelist.txt";

pub struct NTPServerList{
    pub list:Vec<String>,
    file_path:String,   
}

impl NTPServerList {
    pub fn default() -> Self{
        let mut vec = Vec::new();
        vec.push("ptbtime1.ptb.de".to_string());

        Self { list: vec, file_path: NTP_FILE_PATH.to_string()}
    }

    pub fn new(vec:Vec<String>, file_path:&str) -> Self {
        Self { list: vec, file_path:file_path.to_string() }
    }
    
    pub fn init (&mut self) -> Result<usize, Error>{
        let file = File::open(&self.file_path).unwrap();
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let server_address = line.unwrap();
            self.list.push(server_address);
        }

        let count = self.list.len();

        if count > 0 {
            return Ok(count);
        }
        else{
            return Err(Error::new(ErrorKind::InvalidInput, "Can't load ntf servers"));
        }
    }
}


pub fn get_ntp_time(npt_server:&str) -> Result<SystemTime,Error>{

    let client = SntpClient::new();

    let result = match client.synchronize(npt_server) {
        Ok(value) => value,
        Err(_err) => return Ok(SystemTime::now())
    };   

    let delta = result.clock_offset().abs_as_std_duration().unwrap();

    println!("{:>30}:: delta: {}", npt_server, delta.as_secs_f64());

    return Ok(SystemTime::now());
}