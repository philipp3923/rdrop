use std::{io::{ErrorKind, Error, BufReader, Write, BufRead}, fs::{FileType, metadata, File, OpenOptions, self}, path::{Path}, collections::HashMap};
use regex::{Regex};

use chrono::Utc;

use crate::{hash::{hash::{Hash, get_hash}}, error::error::RErrorKind};
use crate::error::error::RError;

pub const USER_HASH:&str = "0123456789abcdef";
pub const CHUNK_HASH_TYPE:Hash = Hash::SIPHASH24;
pub const CHUNK_SIZE:usize = 1024 * 1024*1;
pub const BUFFER_SIZE:usize = 1024 * 1024;
pub const LOGGER_REGEX:&str = r"\[(\d{2}\.\d{2}\.\d{4} \- \d{2}:\d{2}:\d{2}\.\d{3})\][\t\f\v ]*-[\t\f\v ]*\[([a-zA-Z0-9]+)\][\t\f\v ]*-[\t\f\v ]*\[(SHA256|SHA512|MD5|SIPHASH24)\][\t\f\v ]*-[\t\f\v ]*\[([a-zA-Z0-9]+)\][\t\f\v ]*-[\t\f\v ]*\[(\d+)\][\t\f\v ]*-[\t\f\v ]*\[(\d+)\][\t\f\v ]*-[\t\f\v ]*\[(\d+) bytes\][\t\f\v ]*(-[\t\f\v ]*\[(SHA256|SHA512|MD5|SIPHASH24)\][\t\f\v ]*-[\t\f\v ]*\[([a-zA-Z0-9]+)\])?";

#[derive(Debug)]
pub struct LogEntry {
    string_ts: String,
    user_hash: String,
    pub file_hash: String,
    pub file_hash_alg: Hash,
    pub chunk_part: u64,
    pub max_part: u64,
    pub chunk_size: u64,
    pub chunk_hash: Option<String>,
    pub chunk_hash_alg: Option<Hash>,
}

impl LogEntry {
    pub fn new( string_ts: String, user_hash: String,file_hash: String, file_hash_alg: Hash, chunk_part: u64, max_part:u64, chunk_size: u64, chunk_hash: Option<String>, chunk_hash_alg: Option<Hash>) ->
        Self {
            Self {
                string_ts,
                user_hash,
                file_hash,
                file_hash_alg,
                chunk_part,
                chunk_size,
                max_part,
                chunk_hash,
                chunk_hash_alg,
            }
        }
}


#[repr(u8)]
pub enum HeaderByte{
    SendData = 0b00000000,
    SendOffer = 0b00000001,
    SendOrder = 0b00000010,
}

impl HeaderByte{
    pub fn to_byte_arr(&self) -> Vec<u8>{
        let mut vec = vec![0;8];
        match self{
            HeaderByte::SendOffer => {
                vec[7] = 1;
            },
            HeaderByte::SendOrder => {
                vec[6] = 1;
            },
            _ => {},
        }
        return vec;
    }
    pub fn to_u8(&self) -> u8{
        match self{
            HeaderByte::SendData => 0b00000000,
            HeaderByte::SendOrder => 0b00000010,
            HeaderByte::SendOffer => 0b00000001,
        }
    }
}

pub struct AppSettings{
    pub buffer_size:usize,
    pub log_regex: String,
    pub anonymous_mode: bool,
    pub user_hash: String,
    pub file_hash: Hash,
    pub chunk_hash: Option<Hash>,
    pub chunk_size: usize,
    pub output_dir: String,
    pub input_dir: String,
    pub atm_file_name:Option<String>,
}

impl AppSettings {
    pub fn default() -> Self {
        Self { buffer_size: BUFFER_SIZE, log_regex: LOGGER_REGEX.to_string(), anonymous_mode:true, file_hash:Hash::SIPHASH24, chunk_hash:Some(Hash::SIPHASH24), chunk_size: CHUNK_SIZE, output_dir:"./output".to_string(), input_dir:"./input".to_string(), user_hash:"0123456789ABCDEF".to_string(), atm_file_name:None }
    }
}






#[derive(Debug)]
pub struct Header{
    pub header_length:usize,
    pub third_byte:usize,
    pub user_pos_s:usize,
    pub user_pos_e:usize,
    pub chunk_length_pos_s:usize,
    pub chunk_length_pos_e:usize,
    pub file_hash_pos_s:usize,
    pub file_hash_pos_e:usize,
    pub chunk_max_pos_s:usize,
    pub chunk_max_pos_e:usize,
    pub chunk_pos_s:usize,
    pub chunk_pos_e:usize,
    pub chunk_hash_pos_s:Option<usize>,
    pub chunk_hash_pos_e:Option<usize>,
    pub fix_header: Vec<u8>
}

impl Header{
    fn new(
        header_length: usize,
        third_byte:usize,
        user_pos_s: usize,
        user_pos_e: usize,
        chunk_length_pos_s: usize,
        chunk_length_pos_e: usize,
        file_hash_pos_s: usize,
        file_hash_pos_e: usize,
        chunk_max_pos_s: usize,
        chunk_max_pos_e: usize,
        chunk_pos_s: usize,
        chunk_pos_e: usize,
        chunk_hash_pos_s: Option<usize>,
        chunk_hash_pos_e: Option<usize>,
        fix_header: Vec<u8>
    ) -> Self {
        Self {
            header_length, 
            third_byte,
            user_pos_s,
            user_pos_e,
            chunk_length_pos_s,
            chunk_length_pos_e,
            file_hash_pos_s,
            file_hash_pos_e,
            chunk_max_pos_s,
            chunk_max_pos_e,
            chunk_pos_s,
            chunk_pos_e,
            chunk_hash_pos_s,
            chunk_hash_pos_e,
            fix_header,
        }
    }
}


 pub struct FileData{
    path:String,
    pub name:String,
    pub size:u64,
    extension:FileType,
    pub file_hash:Option<String>
}

impl FileData{
    pub fn new(path:&str, name:String, size:u64, extension:FileType, file_hash:Option<String>) -> Self{
        Self{
            path: path.to_string(),
            name: name.to_string(),
            size: size,
            extension:extension,
            file_hash:file_hash,
        }
    }
} 

#[derive(Debug)]
pub struct HeaderData{
    pub user_hash:String,
    pub file_hash:String,
    pub chunk_hash:Option<String>,
    pub user_hash_alg:Hash,
    pub file_hash_alg:Hash,
    pub chunk_hash_alg:Option<Hash>,
    pub chunk_length:usize,
    pub chunk_pos:u64,
    pub chunk_max:u64
}

impl HeaderData {
    pub fn new(user_hash: String, file_hash: String, chunk_hash: String ,chunk_length: usize, chunk_pos: u64, chunk_max: u64,) -> Result<Self,RError> {
        let user_hash_alg = match user_hash.len() {
            16 => Hash::SIPHASH24,
            32 => Hash::MD5,
            64 => Hash::SHA256,
            128 => Hash::SHA512,
            _ => return Err(RError::new(RErrorKind::ConvertionError, &format!("Invalid user-hash length: {}", user_hash.len()))),
        };
        let file_hash_alg = match file_hash.len() {
            16 => Hash::SIPHASH24,
            32 => Hash::MD5,
            64 => Hash::SHA256,
            128 => Hash::SHA512,
            _ => return Err(RError::new(RErrorKind::ConvertionError, &format!("Invalid user-hash length: {}", user_hash.len()))),
        };

        let chunk_hash_alg = match chunk_hash.len() {
            16 => Some(Hash::SIPHASH24),
            32 => Some(Hash::MD5),
            64 => Some(Hash::SHA256),
            128 => Some(Hash::SHA512),
            _ => None,
        };

        let c_hash;
        if chunk_hash_alg.is_none(){
            c_hash = None;
        }
        else{
            c_hash = Some(chunk_hash);
        }
          
        Ok(HeaderData {
            user_hash,
            file_hash,
            chunk_hash:c_hash,
            user_hash_alg,
            file_hash_alg,
            chunk_hash_alg,
            chunk_length,
            chunk_pos,
            chunk_max,
        })
    }
}



//append header to byte-stream
/*
    Used 85
*/
pub fn append_header(byte_vector:Vec<u8>, header_type:HeaderByte) -> Vec<u8>{

    let mut byte_vec:Vec<u8> =Vec::new();
    byte_vec.push(header_type.to_u8());
    byte_vec.extend(byte_vector.iter());
    return byte_vec;
}




//calc how many chunks with certain file size will be produced
/*
    aktive 85
*/
pub fn calc_chunk_count(chunk_size:usize, file_size:u64) -> Result<u64,RError> {

    let chunk_size:u64 = match chunk_size.try_into() {
        Ok(value) => value,
        Err(_) => {
            return Err(RError::new(RErrorKind::ConvertionError, "Error while converting usize to u64."));
        }
    };

    let mut full_val = file_size / chunk_size;
    
    if file_size % chunk_size != 0 {
        full_val += 1;
    }

    return Ok(full_val);
}




//write header
pub fn read_header(header_vec:&Vec<u8>) -> Result<Header, RError>{
    let bitmask_chunk_size:u8  = 0b10000000;
    let bitmask_file_hash  = 0b00011000;
    let bitmask_chunk_count:u8 = 0b01100000;
    let bitmask_chunk_hash:u8  = 0b00000111; 
    let mut header = Header::new(0,0,3,10,11,0,0,0,0,0,0,0,Some(0),Some(0),Vec::new());

    header.header_length = header_vec.get(1).unwrap().clone() as usize;

    header.third_byte = header_vec.get(2).unwrap().clone() as usize;

    let mut length = 11;
    
    header.chunk_length_pos_s = length;
    match header.third_byte as u8 & bitmask_chunk_size {
        0b00000000 => {// das erste bit ist 0
            length = length + 3
        },   
        0b10000000 => {// das erste bit ist 1
            length = length + 4
        },
        _ => {
            return Err(RError::new(RErrorKind::ReadHeaderError, "Can't map headerbit in bit 3 for ReadData - calc chunk_size."));
        }
    };
    header.chunk_length_pos_e = length - 1;


    header.file_hash_pos_s = length;
    match header.third_byte as u8 & bitmask_file_hash {
        0b00000000 => {// die ersten beiden Bits sind 00
            length = length + 8;
        },   
        0b00001000 => {// die ersten beiden Bits sind 01
            length = length + 16;
        },   
        0b00010000 => {// die ersten beiden Bits sind 10
            length = length + 32;
        },   
        0b00011000 => {// die ersten beiden Bits sind 11
            length = length + 64;
        },
        _ => {
            return Err(RError::new(RErrorKind::ReadHeaderError, "Can't map headerbit in bit 3 for ReadData - Calc file_hash."));
        }
    };
    header.file_hash_pos_e = length - 1;


    header.chunk_max_pos_s = length;
    match header.third_byte as u8 & bitmask_chunk_count {
        0b00000000 => {// die ersten beiden Bits sind 00
            length = length + 1;
        },   
        0b00100000 => {// die ersten beiden Bits sind 01
            length = length + 2;
        },   
        0b01000000 => {// die ersten beiden Bits sind 10
            length = length + 3;
        },   
        0b01100000 => {// die ersten beiden Bits sind 11
            length = length + 4;
        },
        _ => {
            return Err(RError::new(RErrorKind::ReadHeaderError, "Can't map headerbit in bit 3 for ReadData - Calc chunk_count"));
        }
    };
    header.chunk_max_pos_e = length - 1;   

    header.chunk_pos_s = length;
    length = length + 1 + (header.chunk_max_pos_e - header.chunk_max_pos_s);
    header.chunk_pos_e = length -1;    
    
    header.chunk_hash_pos_s = Some(length);

    match header.third_byte as u8 & bitmask_chunk_hash {
        0b00000000 => {
            header.chunk_hash_pos_s = None;
            header.chunk_hash_pos_e = None;
        },   
        0b00000100 => {
            length = length + 8;
            header.chunk_hash_pos_e = Some(length - 1);
        },   
        0b00000101 => {
            length = length + 16;
            header.chunk_hash_pos_e = Some(length - 1);
        },   
        0b00000110 => {
            length = length + 32;
            header.chunk_hash_pos_e = Some(length - 1);
        },
        0b00000111 => {
            length = length + 64;
            header.chunk_hash_pos_e = Some(length - 1);
        },
        _ => {
            return Err(RError::new(RErrorKind::ReadHeaderError, "Can't map headerbit in bit 3 for ReadData - calc chunk_hash."));
        }
    };

    if length != header.header_length{

        return Err(RError::new(RErrorKind::ReadHeaderError, "Header length is not right."));
    }


    header.fix_header = header_vec.clone();   

    return Ok(header);

}



pub fn extract_header_data(header:&Header) -> Result<HeaderData,RError>{

    let mut user_hash:String = "".to_string();
    let mut chunk_length:u32 = 0;
    let mut file_hash:String = "".to_string();
    let mut chunk_max:u32 = 0;
    let mut chunk_pos:u32 = 0;
    let mut chunk_hash:String = "".to_string();


    for i in header.user_pos_s..=header.user_pos_e {
        
        let val = header.fix_header[i];
        user_hash = format!("{}{:02X}", user_hash, val);
    }

    for i in header.chunk_length_pos_s..=header.chunk_length_pos_e {
        
        let val = header.fix_header[i] as u32;
        chunk_length = (chunk_length << 8) | (val)
    }

    for i in header.file_hash_pos_s..=header.file_hash_pos_e {
        
        let val = header.fix_header[i];
        file_hash = format!("{}{:02X}", file_hash, val);
    }

    for i in header.chunk_max_pos_s..=header.chunk_max_pos_e {
        
        let val = header.fix_header[i] as u32;
        chunk_max = (chunk_max << 8) | (val)
    }

    for i in header.chunk_pos_s..=header.chunk_pos_e {
        
        let val = header.fix_header[i] as u32;
        chunk_pos = (chunk_pos << 8) | (val)
    }

    if header.chunk_hash_pos_s.is_some() && header.chunk_hash_pos_e.is_some(){
        for i in header.chunk_hash_pos_s.unwrap()..=header.chunk_hash_pos_e.unwrap() { 
            let val = header.fix_header[i];
            chunk_hash = format!("{}{:02X}", chunk_hash, val);   
        }
    }
    return HeaderData::new(user_hash, file_hash, chunk_hash, chunk_length as usize, chunk_pos as u64, chunk_max as u64);

}











//write header
pub fn create_header(file_length: u64, chunk_count:u64, file_hash_type:&Hash, chunk_hash_type:&Option<Hash>) -> Header{

    let mut header = Header::new(0,0,3,10,11,0,0,0,0,0,0,0,Some(0),Some(0),Vec::new());

    let mut length:usize = 11;


    //setzte 1 bit im dritten byte 1 wenn 4 bytes benötigt werden für die länge
    let mut third_byte = [0;8];

    header.chunk_length_pos_s = length;
    if file_length > 2^24 - 1 {
        third_byte[0] = 1;
        length = length + 4;
    }
    else {
        length = length + 3;
    }
    header.chunk_length_pos_e = length - 1;

    header.file_hash_pos_s = length;
    //setze bytes für file_hash_size
    match file_hash_type{
        Hash::SIPHASH24 => {
            third_byte[3] = 0;
            third_byte[4] = 0;
            length = length + 8;
        },
        Hash::MD5 => {
            third_byte[3] = 0;
            third_byte[4] = 1;
            length = length + 16;
        },
        Hash::SHA256 => {
            third_byte[3] = 1;
            third_byte[4] = 0;
            length = length + 32;
        },
        Hash::SHA512 => {
            third_byte[3] = 1;
            third_byte[4] = 1;
            length = length + 64;
        }
    }
    header.file_hash_pos_e = length - 1;

    header.chunk_max_pos_s = length;
    //setzte die länge für size bytes


    if chunk_count < 2u64.pow(8) {
        third_byte[1] = 0;
        third_byte[2] = 0;
        length = length + 1;
    }
    else if chunk_count < 2u64.pow(16) {
        third_byte[1] = 0;
        third_byte[2] = 1;
        length = length + 2;
    }
    else if chunk_count < 2u64.pow(24) {
        third_byte[1] = 1;
        third_byte[2] = 0;
        length = length + 3;
    }
    else if chunk_count < 2u64.pow(32) {
        third_byte[1] = 1;
        third_byte[2] = 1;
        length = length + 4;
    }
    header.chunk_max_pos_e = length - 1;

    header.chunk_pos_s = length;
    length = length + 1 + (header.chunk_max_pos_e - header.chunk_max_pos_s);
    header.chunk_pos_e = length -1;    
    
    header.chunk_hash_pos_s = Some(length);
    let tmp_l = length;
    match chunk_hash_type{
        Some(Hash::SIPHASH24) => {
            third_byte[5] = 1;
            third_byte[6] = 0;
            third_byte[7] = 0;
            length = length + 8;
        },
        Some(Hash::MD5) => {
            third_byte[5] = 1;
            third_byte[6] = 0;
            third_byte[7] = 1;
            length = length + 16;
        },
        Some(Hash::SHA256) => {
            third_byte[5] = 1;
            third_byte[6] = 1;
            third_byte[7] = 0;
            length = length + 32;
        },
        Some(Hash::SHA512) => {
            third_byte[5] = 1;
            third_byte[6] = 1;
            third_byte[7] = 1;
            length = length + 64;
        },
        None => {
            third_byte[5] = 0;
            third_byte[6] = 0;
            third_byte[7] = 0;
        }
    }

    if tmp_l == length{
        header.chunk_hash_pos_s = None;
        header.chunk_hash_pos_e = None;
    }
    else {
        header.chunk_hash_pos_e = Some(length - 1);
    }


    let mut byte:u8 = 0;
    for i in 0..8 {
        if third_byte[i] != 0 {
            byte |= 1 << (7 - i);
        }
    }
    
    let mut fix_header = vec![0;length];

    fix_header[0] = 0 as u8;
    fix_header[1] = length as u8;
    fix_header[2] = byte as u8;

    header.fix_header = fix_header;   

    header.header_length = length;
    header.third_byte = byte as usize;

    return header;


}


pub fn write_in_header(header:&mut Vec<u8>, value:u64, start_pos:usize, end_pos:usize){

    let value_bytes = value.to_be_bytes();

    let byte_count = end_pos - start_pos + 1;

    for i in 0..byte_count {
        header[end_pos - i] = value_bytes[value_bytes.len() - 1 - i];
    }
}

pub fn write_hex_in_header(header:&mut Vec<u8>, value:&str, start_pos:usize, end_pos:usize) -> Result<(),RError>{

    if end_pos - start_pos + 1 < value.len() / 2 {
        return Err(RError::new(RErrorKind::InputOutputError, "Can't write string in byte-vector. Reason: vector to short."));
    }


    //let num = u64::from_str_radix(value, 16).map_err(|_err| RError::new( RErrorKind::ConvertionError, "Failed to parse hex string."))?;
    //let bytes = num.to_be_bytes();

    let mut bytes = Vec::new();
    for i in (0..value.len()).step_by(2) {
        let byte = u8::from_str_radix(&value[i..i+2], 16).map_err(|_| RError::new(RErrorKind::ConvertionError, "Failed to parse hex string."))?;
        bytes.push(byte);
    }


    for i in 0..bytes.len() {
        header[start_pos + i] = bytes[i];
    }

    return Ok(());

}


pub fn separate_header(data:&Vec<u8>) -> Result<(Vec<u8>, Vec<u8>), RError>{

    let first_byte = data[0];

    if first_byte != 0 {
        return Err(RError::new(RErrorKind::InputOutputError, "Header does not comply with the guidelines and cannot be read."))
    }

    let second_byte = data[1];

    let (header, data) = data.split_at(second_byte as usize);

    let header = header.to_vec();
    let data = data.to_vec();

    return Ok((header,data));
}


pub fn read_send_header(byte_vec:&Vec<u8>) -> Result<HeaderData, RError>{

    let new_header = read_header(&byte_vec)?;
    let header_data = extract_header_data(&new_header);

    return header_data;
}

//get file data from path
pub fn load_file_data(filepath:&str) -> Result<FileData, Error>{
    
    let metadata = match metadata(filepath){
        Ok(metadata) => metadata,
        Err(_) => return Err(Error::new(ErrorKind::InvalidInput, "Path is not a file or directory")),
    };

    if metadata.is_dir() {
        //return error, only files can be splitted
        return Err(Error::new(ErrorKind::InvalidInput, "Path is a directory. Can only send Files (includes .zip)"));
    }

    let extension = metadata.file_type();
    let name = Path::new(filepath).file_name().unwrap().to_string_lossy().to_string();
    let size = metadata.len();

    let file_data = FileData::new(filepath, name, size, extension, None);
    
    return Ok(file_data);
}


pub fn check_chunk_hash(header_hash:&Option<String>, calc_hash_alg:&Option<Hash>, byte_vec:&Vec<u8>) -> bool{

    if let Some(hash_alg) = calc_hash_alg{
        let hash = get_hash(byte_vec, &hash_alg);

        if let Some(h_hash) = header_hash{

            if h_hash.to_lowercase() == hash.to_lowercase(){
                return true;
            }
            else{
                return false;
            }
        }
    }
    true
}

/*
    Aktuell schreibt log-file
*/
pub fn write_to_log_file(path: &str, user_hash: &str, parent_hash_alg: &Hash, part_num: u64, max_num:u64, part_size:u64, parent_hash:&str, part_hash_alg: &Option<Hash>, part_hash: &Option<String>) -> Result<bool,Error> {
    
    let time = Utc::now().format("%d.%m.%Y - %H:%M:%S%.3f").to_string();
    
    
    let opt_hash = match (part_hash_alg, part_hash) {
        (Some(alg), Some(hash)) => {
            format!(" - [{}] - [{}]", alg.to_string(), hash)
        },
        _ => "".to_string(),
    };
        
    let log_line = format!("[{}] - [{}] - [{}] - [{}] - [{}] - [{}] - [{} bytes]{}", time, user_hash, parent_hash_alg.to_string(), parent_hash, part_num, max_num, part_size, opt_hash);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path).unwrap();
    writeln!(file, "{}", log_line).unwrap();

    Ok(true)
}


pub fn validate_log_file(vec:&Vec<LogEntry>) -> (u64, u64){

    let mut missing_vec = Vec::new();
    let max_count = vec[0].max_part;
    let mut found_values = vec![false; max_count as usize];


    for entry in vec {
        if entry.chunk_part <= max_count {
            found_values[entry.chunk_part as usize - 1] = true;
        }
    }

    for (i, &found) in found_values.iter().enumerate() {
        if !found {
            missing_vec.push(i as u64 + 1);
        }
    }

    match missing_vec.len(){
        0 => return (0,0),
        1 => return (missing_vec[0], missing_vec[0]),
        _ => {
            let x = missing_vec.len() - 1;
            return (missing_vec[0], missing_vec[x])
        }
    }
}



pub fn read_log_file(path: &str, buffer_size: usize, regex: &str) -> Result<Vec<LogEntry>, Error> {

    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return Err(Error::new(ErrorKind::InvalidInput, "Path can not be opended.")),
    };

    let buf_reader = BufReader::with_capacity(buffer_size, file);

    let mut result = HashMap::new();

    let regex = Regex::new(regex).unwrap();

    for entry in buf_reader.lines() {
        
        let line = match entry {
            Ok(line) => line,
            Err(_) => continue,
        };

        if let Some(captures) = regex.captures(&line) {

            let chunk_hash_alg = if captures.get(8).is_some() && captures.get(9).is_some() {
                let chunk_hash_alg_tmp = match &captures[9]{
                    "SIPHASH24" => Hash::SIPHASH24,
                    "SHA512" => Hash::SHA512,
                    "MD5" => Hash::MD5,
                    "SHA256" => Hash::SHA256,
                    _ => return Err(Error::new(ErrorKind::InvalidInput, "Hash-Algorithm not found")),

                };
                Some(chunk_hash_alg_tmp)
            } else {
                None
            };
    
            let chunk_hash = if captures.get(8).is_some() && captures.get(10).is_some() {
                Some(captures[10].to_owned())
            } else {
                None
            };

            let file_hash_alg = match &captures[3]{
                "SIPHASH24" => Hash::SIPHASH24,
                "SHA512" => Hash::SHA512,
                "MD5" => Hash::MD5,
                "SHA256" => Hash::SHA256,
                _ => return Err(Error::new(ErrorKind::InvalidInput, "Hash-Algorithm not found")),
            };

            let log_entry = LogEntry {
                string_ts: captures[1].to_owned(),
                user_hash: captures[2].to_owned(),
                file_hash_alg: file_hash_alg,
                file_hash: captures[4].to_owned(),
                chunk_part: captures[5].parse::<u64>().unwrap(),
                max_part: captures[6].parse::<u64>().unwrap(),
                chunk_size: captures[7].parse::<u64>().unwrap(),
                chunk_hash_alg: chunk_hash_alg,
                chunk_hash: chunk_hash,
            };
            result.insert(log_entry.chunk_part, log_entry);
        }
    }         
    let vec = result.into_iter().map(|(_, le)| le).collect();
    return Ok(vec);

}

pub fn get_filename_from_dir(dir:&str) -> Result<String, RError>{
    
    for entry in fs::read_dir(dir).map_err(|_err| RError::new( RErrorKind::InputOutputError, "Can't open directory while trying to read the logfile for the filename."))? {
        if let Ok(entry) = entry {

            let file_name = entry.file_name().into_string().map_err(|_err| RError::new( RErrorKind::InputOutputError, "Can't read filename from dir while trying to read the logfile for the filename."))?;

            if file_name.ends_with(".rdroplog") && entry.file_type().map_err(|_err| RError::new( RErrorKind::InputOutputError, "Can't read file_type of file_entry in output_dir while trying to read the logfile for the filename."))?.is_file() {
                let file_name = file_name.trim_end_matches(".rdroplog").to_string();
                
                return Ok(file_name)
            }
        }
    }
    return Err(RError::new(RErrorKind::InputOutputError, "Can't read file_name from logfile"));
}


pub fn validate_file(output_dir:&str, file_hash:&str) -> Result<(u64, u64),RError>{

    let outpath = format!("{}/{}",&output_dir, &file_hash);

    let file_name = get_filename_from_dir(&outpath).map_err(|err| RError::new( RErrorKind::InputOutputError, &err.to_string()))?;

    let logfile_path = format!("{}/{}.rdroplog", outpath, &file_name);

    let mut log_entry_vec = read_log_file(&logfile_path, BUFFER_SIZE, LOGGER_REGEX).map_err(|err| RError::new( RErrorKind::InputOutputError, &err.to_string()))?;

    let (startpos, endpos) = validate_log_file(&mut log_entry_vec);

    return Ok((startpos, endpos));

}