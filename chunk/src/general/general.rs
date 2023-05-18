use regex::Regex;
use std::{
    collections::HashMap,
    fs::{metadata, File, FileType, OpenOptions},
    io::{BufRead, BufReader, Error, ErrorKind, Write},
    path::Path,
};

use chrono::Utc;

use crate::error::error::RError;
use crate::{
    error::error::RErrorKind,
    hash::hash::{get_hash, Hash},
};

pub const USER_HASH: &str = "0123456789abcdef";
pub const CHUNK_HASH_TYPE: Hash = Hash::SIPHASH24;
pub const CHUNK_SIZE: usize = 1024 * 300;
pub const BUFFER_SIZE: usize = 1024 * 300;
pub const LOGGER_REGEX: &str = r"\[(\d{2}\.\d{2}\.\d{4} \- \d{2}:\d{2}:\d{2}\.\d{3})\][\t\f\v ]*-[\t\f\v ]*\[([a-fA-F0-9]+)\][\t\f\v ]*-[\t\f\v ]*\[(SHA256|SHA512|MD5|SIPHASH24)\][\t\f\v ]*-[\t\f\v ]*\[([a-fA-F0-9]+)\][\t\f\v ]*-[\t\f\v ]*\[(\d+)\][\t\f\v ]*-[\t\f\v ]*\[(\d+)\][\t\f\v ]*-[\t\f\v ]*\[(\d+) bytes\][\t\f\v ]*(-[\t\f\v ]*\[(SHA256|SHA512|MD5|SIPHASH24)\][\t\f\v ]*-[\t\f\v ]*\[([a-fA-F0-9]+)\])?";
pub const STOP_REGEX: &str = r"\[([a-fA-F0-9]+)\]";

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
    pub fn new(
        string_ts: String,
        user_hash: String,
        file_hash: String,
        file_hash_alg: Hash,
        chunk_part: u64,
        max_part: u64,
        chunk_size: u64,
        chunk_hash: Option<String>,
        chunk_hash_alg: Option<Hash>,
    ) -> Self {
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
pub enum HeaderByte {
    SendData = 0b00000000,
    SendOffer = 0b00000001,
    SendOrder = 0b00000010,
}

impl HeaderByte {
    pub fn to_byte_arr(&self) -> Vec<u8> {
        let mut vec = vec![0; 8];
        match self {
            HeaderByte::SendOffer => {
                vec[7] = 1;
            }
            HeaderByte::SendOrder => {
                vec[6] = 1;
            }
            _ => {}
        }
        return vec;
    }
    pub fn to_u8(&self) -> u8 {
        match self {
            HeaderByte::SendData => 0b00000000,
            HeaderByte::SendOrder => 0b00000010,
            HeaderByte::SendOffer => 0b00000001,
        }
    }
}

//deprecated
pub struct AppSettings {
    pub buffer_size: usize,
    pub log_regex: String,
    pub anonymous_mode: bool,
    pub user_hash: String,
    pub file_hash: Hash,
    pub chunk_hash: Option<Hash>,
    pub chunk_size: usize,
    pub output_dir: String,
    pub input_dir: String,
    pub atm_file_name: Option<String>,
}

impl AppSettings {
    pub fn default() -> Self {
        Self {
            buffer_size: BUFFER_SIZE,
            log_regex: LOGGER_REGEX.to_string(),
            anonymous_mode: true,
            file_hash: Hash::SIPHASH24,
            chunk_hash: Some(Hash::SIPHASH24),
            chunk_size: CHUNK_SIZE,
            output_dir: "./output".to_string(),
            input_dir: "./input".to_string(),
            user_hash: "0123456789ABCDEF".to_string(),
            atm_file_name: None,
        }
    }
}

#[derive(Debug)]
pub struct Header {
    pub header_length: usize,
    pub third_byte: usize,
    pub user_pos_s: usize,
    pub user_pos_e: usize,
    pub chunk_length_pos_s: usize,
    pub chunk_length_pos_e: usize,
    pub file_hash_pos_s: usize,
    pub file_hash_pos_e: usize,
    pub chunk_max_pos_s: usize,
    pub chunk_max_pos_e: usize,
    pub chunk_pos_s: usize,
    pub chunk_pos_e: usize,
    pub chunk_hash_pos_s: Option<usize>,
    pub chunk_hash_pos_e: Option<usize>,
    pub fix_header: Vec<u8>,
}

impl Header {
    fn new(
        header_length: usize,
        third_byte: usize,
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
        fix_header: Vec<u8>,
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

pub struct FileData {
    path: String,
    pub name: String,
    pub size: u64,
    extension: FileType,
    pub file_hash: Option<String>,
}

impl FileData {
    pub fn new(
        path: &str,
        name: String,
        size: u64,
        extension: FileType,
        file_hash: Option<String>,
    ) -> Self {
        Self {
            path: path.to_string(),
            name: name.to_string(),
            size: size,
            extension: extension,
            file_hash: file_hash,
        }
    }
}

#[derive(Debug)]
pub struct HeaderData {
    pub user_hash: String,
    pub file_hash: String,
    pub chunk_hash: Option<String>,
    pub user_hash_alg: Hash,
    pub file_hash_alg: Hash,
    pub chunk_hash_alg: Option<Hash>,
    pub chunk_length: usize,
    pub chunk_pos: u64,
    pub chunk_max: u64,
}

impl HeaderData {
    pub fn new(
        user_hash: String,
        file_hash: String,
        chunk_hash: String,
        chunk_length: usize,
        chunk_pos: u64,
        chunk_max: u64,
    ) -> Result<Self, RError> {
        let user_hash_alg = match user_hash.len() {
            16 => Hash::SIPHASH24,
            32 => Hash::MD5,
            64 => Hash::SHA256,
            128 => Hash::SHA512,
            _ => {
                return Err(RError::new(
                    RErrorKind::ConvertionError,
                    &format!("Invalid user-hash length: {}", user_hash.len()),
                ))
            }
        };
        let file_hash_alg = match file_hash.len() {
            16 => Hash::SIPHASH24,
            32 => Hash::MD5,
            64 => Hash::SHA256,
            128 => Hash::SHA512,
            _ => {
                return Err(RError::new(
                    RErrorKind::ConvertionError,
                    &format!("Invalid user-hash length: {}", user_hash.len()),
                ))
            }
        };

        let chunk_hash_alg = match chunk_hash.len() {
            16 => Some(Hash::SIPHASH24),
            32 => Some(Hash::MD5),
            64 => Some(Hash::SHA256),
            128 => Some(Hash::SHA512),
            _ => None,
        };

        let c_hash;
        if chunk_hash_alg.is_none() {
            c_hash = None;
        } else {
            c_hash = Some(chunk_hash);
        }

        Ok(HeaderData {
            user_hash,
            file_hash,
            chunk_hash: c_hash,
            user_hash_alg,
            file_hash_alg,
            chunk_hash_alg,
            chunk_length,
            chunk_pos,
            chunk_max,
        })
    }
}

/// Retrieves file data from the specified file path.
///
/// # Arguments
///
/// * `filepath` - The path to the file.
///
/// # Returns
///
/// The function returns a tuple containing the file, file name, and file size if successful.
///
/// # Errors
///
/// The function can return an error if the specified path is not a valid file or directory, or if there is an error opening the file.
///
pub fn get_file_data(filepath: &str) -> Result<(File, String, u64), Error> {
    let metadata = match metadata(filepath) {
        Ok(metadata) => metadata,
        Err(_) => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Path is not a file or directory",
            ))
        }
    };

    if metadata.is_dir() {
        //return error, only files can be splitted
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Path is a directory. Can only send Files (includes .zip)",
        ));
    }

    let file = File::open(filepath)?;
    let name = Path::new(filepath)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let size = metadata.len();

    return Ok((file, name, size));
}



/// Calculates the number of chunks required to split a file based on its size.
///
/// # Arguments
///
/// * `file_size` - The size of the file in bytes.
///
/// # Returns
///
/// The function returns the number of chunks required to split the file.
///
pub fn get_chunk_count(file_size: u64) -> u64 {
    let chunk_size = CHUNK_SIZE as u64;

    let mut full_val = file_size / chunk_size;

    if file_size % chunk_size != 0 {
        full_val += 1;
    }

    return full_val;
}


/// Appends a header byte to the beginning of a byte vector.
///
/// # Arguments
///
/// * `byte_vector` - The byte vector to which the header byte will be appended.
/// * `header_type` - The header byte value to append.
///
/// # Returns
///
/// The function returns a new byte vector with the header byte appended.
///
pub fn append_header(byte_vector: Vec<u8>, header_type: HeaderByte) -> Vec<u8> {
    let mut byte_vec: Vec<u8> = Vec::new();
    byte_vec.push(header_type.to_u8());
    byte_vec.extend(byte_vector.iter());
    return byte_vec;
}

/// Calculates the number of chunks needed to split a file based on the chunk size and file size.
///
/// # Arguments
///
/// * `chunk_size` - The size of each chunk in bytes.
/// * `file_size` - The size of the file in bytes.
///
/// # Returns
///
/// The function returns the number of chunks as a `Result` where `Ok` contains the calculated number of chunks,
/// or `Err` contains an `RError` indicating an error during conversion or calculation.
///
pub fn calc_chunk_count(chunk_size: usize, file_size: u64) -> Result<u64, RError> {
    let chunk_size: u64 = match chunk_size.try_into() {
        Ok(value) => value,
        Err(_) => {
            return Err(RError::new(
                RErrorKind::ConvertionError,
                "Error while converting usize to u64.",
            ));
        }
    };

    let mut full_val = file_size / chunk_size;

    if file_size % chunk_size != 0 {
        full_val += 1;
    }

    return Ok(full_val);
}



/// Reads the header information from a byte vector and constructs a `Header` struct.
///
/// # Arguments
///
/// * `header_vec` - The byte vector containing the header information.
///
/// # Returns
///
/// The function returns a `Result` where `Ok` contains the constructed `Header` struct,
/// or `Err` contains an `RError` indicating an error during header parsing.
///
pub fn read_header(header_vec: &Vec<u8>) -> Result<Header, RError> {
    //bitmasks for length of different data in header
    let bitmask_chunk_size: u8 = 0b10000000;
    let bitmask_file_hash = 0b00011000;
    let bitmask_chunk_count: u8 = 0b01100000;
    let bitmask_chunk_hash: u8 = 0b00000111;
    let mut header = Header::new(
        0,
        0,
        3,
        10,
        11,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        Some(0),
        Some(0),
        Vec::new(),
    );

    header.header_length = header_vec.get(1).unwrap().clone() as usize;

    header.third_byte = header_vec.get(2).unwrap().clone() as usize;

    let mut length = 11;

    header.chunk_length_pos_s = length;
    match header.third_byte as u8 & bitmask_chunk_size {
        0b00000000 => {
            // first bit is 0
            length = length + 3
        }
        0b10000000 => {
            // first bit is 1
            length = length + 4
        }
        _ => {
            return Err(RError::new(
                RErrorKind::ReadHeaderError,
                "Can't map headerbit in bit 3 for ReadData - calc chunk_size.",
            ));
        }
    };
    header.chunk_length_pos_e = length - 1;

    header.file_hash_pos_s = length;
    match header.third_byte as u8 & bitmask_file_hash {
        0b00000000 => {
            // Bits 4 and 5 are 00
            length = length + 8;
        }
        0b00001000 => {
            // Bits 4 and 5 are 01
            length = length + 16;
        }
        0b00010000 => {
            // Bits 4 and 5 are 10
            length = length + 32;
        }
        0b00011000 => {
            // Bits 4 and 5 are 11
            length = length + 64;
        }
        _ => {
            return Err(RError::new(
                RErrorKind::ReadHeaderError,
                "Can't map headerbit in bit 3 for ReadData - Calc file_hash.",
            ));
        }
    };
    header.file_hash_pos_e = length - 1;

    header.chunk_max_pos_s = length;
    match header.third_byte as u8 & bitmask_chunk_count {
        0b00000000 => {
            // Bits 2 and 3 are 00
            length = length + 1;
        }
        0b00100000 => {
            // Bits 2 and 3 are 01
            length = length + 2;
        }
        0b01000000 => {
            // Bits 2 and 3 are 10
            length = length + 3;
        }
        0b01100000 => {
            // Bits 2 and 3 are 11
            length = length + 4;
        }
        _ => {
            return Err(RError::new(
                RErrorKind::ReadHeaderError,
                "Can't map headerbit in bit 3 for ReadData - Calc chunk_count",
            ));
        }
    };
    header.chunk_max_pos_e = length - 1;

    header.chunk_pos_s = length;
    length = length + 1 + (header.chunk_max_pos_e - header.chunk_max_pos_s);
    header.chunk_pos_e = length - 1;

    header.chunk_hash_pos_s = Some(length);

    match header.third_byte as u8 & bitmask_chunk_hash {
        0b00000000 => {
            // Bits 6, 7 and 8 are 000
            header.chunk_hash_pos_s = None;
            header.chunk_hash_pos_e = None;
        }
        0b00000100 => {
            // Bits 6, 7 and 8 are 100
            length = length + 8;
            header.chunk_hash_pos_e = Some(length - 1);
        }
        0b00000101 => {
            // Bits 6, 7 and 8 are 101
            length = length + 16;
            header.chunk_hash_pos_e = Some(length - 1);
        }
        0b00000110 => {
            // Bits 6, 7 and 8 are 110
            length = length + 32;
            header.chunk_hash_pos_e = Some(length - 1);
        }
        0b00000111 => {
            // Bits 6, 7 and 8 are 111
            length = length + 64;
            header.chunk_hash_pos_e = Some(length - 1);
        }
        _ => {
            return Err(RError::new(
                RErrorKind::ReadHeaderError,
                "Can't map headerbit in bit 3 for ReadData - calc chunk_hash.",
            ));
        }
    };

    if length != header.header_length {
        return Err(RError::new(
            RErrorKind::ReadHeaderError,
            "Header length is not right.",
        ));
    }

    header.fix_header = header_vec.clone();

    return Ok(header);
}



/// Extracts the header data from a `Header` struct and constructs a `HeaderData` struct.
///
/// # Arguments
///
/// * `header` - The `Header` struct containing the header information.
///
/// # Returns
///
/// The function returns a `Result` where `Ok` contains the constructed `HeaderData` struct,
/// or `Err` contains an `RError` indicating an error during header extraction.
///
pub fn extract_header_data(header: &Header) -> Result<HeaderData, RError> {
    let mut user_hash: String = "".to_string();
    let mut chunk_length: u32 = 0;
    let mut file_hash: String = "".to_string();
    let mut chunk_max: u32 = 0;
    let mut chunk_pos: u32 = 0;
    let mut chunk_hash: String = "".to_string();

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

    if header.chunk_hash_pos_s.is_some() && header.chunk_hash_pos_e.is_some() {
        for i in header.chunk_hash_pos_s.unwrap()..=header.chunk_hash_pos_e.unwrap() {
            let val = header.fix_header[i];
            chunk_hash = format!("{}{:02X}", chunk_hash, val);
        }
    }
    return HeaderData::new(
        user_hash,
        file_hash.to_lowercase(),
        chunk_hash.to_lowercase(),
        chunk_length as usize,
        chunk_pos as u64,
        chunk_max as u64,
    );
}




/// Creates a header based on the provided parameters.
///
/// # Arguments
///
/// * `file_length` - The length of the file in bytes.
/// * `chunk_count` - The number of chunks in the file.
/// * `file_hash_type` - The type of hash algorithm used for file hash.
/// * `chunk_hash_type` - The type of hash algorithm used for chunk hashes (optional).
///
/// # Returns
///
/// The function returns a `Header` struct representing the created header.
///
pub fn create_header(
    file_length: u64,
    chunk_count: u64,
    file_hash_type: &Hash,
    chunk_hash_type: &Option<Hash>,
) -> Header {
    let mut header = Header::new(
        0,
        0,
        3,
        10,
        11,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        Some(0),
        Some(0),
        Vec::new(),
    );

    let mut length: usize = 11;

    let mut third_byte = [0; 8];

    //set 1 bit in third byte 1 when 4 bytes are needed for length
    header.chunk_length_pos_s = length;
    if file_length > 2 ^ 24 - 1 {
        third_byte[0] = 1;
        length = length + 4;
    } else {
        length = length + 3;
    }
    header.chunk_length_pos_e = length - 1;

    header.file_hash_pos_s = length;
    // set bits for file_hash_size
    match file_hash_type {
        Hash::SIPHASH24 => {
            third_byte[3] = 0;
            third_byte[4] = 0;
            length = length + 8;
        }
        Hash::MD5 => {
            third_byte[3] = 0;
            third_byte[4] = 1;
            length = length + 16;
        }
        Hash::SHA256 => {
            third_byte[3] = 1;
            third_byte[4] = 0;
            length = length + 32;
        }
        Hash::SHA512 => {
            third_byte[3] = 1;
            third_byte[4] = 1;
            length = length + 64;
        }
    }
    header.file_hash_pos_e = length - 1;

    header.chunk_max_pos_s = length;
    //set bits for chunk_count bytes
    if chunk_count < 2u64.pow(8) {
        third_byte[1] = 0;
        third_byte[2] = 0;
        length = length + 1;
    } else if chunk_count < 2u64.pow(16) {
        third_byte[1] = 0;
        third_byte[2] = 1;
        length = length + 2;
    } else if chunk_count < 2u64.pow(24) {
        third_byte[1] = 1;
        third_byte[2] = 0;
        length = length + 3;
    } else if chunk_count < 2u64.pow(32) {
        third_byte[1] = 1;
        third_byte[2] = 1;
        length = length + 4;
    }
    header.chunk_max_pos_e = length - 1;

    header.chunk_pos_s = length;
    length = length + 1 + (header.chunk_max_pos_e - header.chunk_max_pos_s);
    header.chunk_pos_e = length - 1;

    //set bits for chunk_hash bytes
    header.chunk_hash_pos_s = Some(length);
    let tmp_l = length;
    match chunk_hash_type {
        Some(Hash::SIPHASH24) => {
            third_byte[5] = 1;
            third_byte[6] = 0;
            third_byte[7] = 0;
            length = length + 8;
        }
        Some(Hash::MD5) => {
            third_byte[5] = 1;
            third_byte[6] = 0;
            third_byte[7] = 1;
            length = length + 16;
        }
        Some(Hash::SHA256) => {
            third_byte[5] = 1;
            third_byte[6] = 1;
            third_byte[7] = 0;
            length = length + 32;
        }
        Some(Hash::SHA512) => {
            third_byte[5] = 1;
            third_byte[6] = 1;
            third_byte[7] = 1;
            length = length + 64;
        }
        None => {
            third_byte[5] = 0;
            third_byte[6] = 0;
            third_byte[7] = 0;
        }
    }

    if tmp_l == length {
        header.chunk_hash_pos_s = None;
        header.chunk_hash_pos_e = None;
    } else {
        header.chunk_hash_pos_e = Some(length - 1);
    }

    let mut byte: u8 = 0;
    for i in 0..8 {
        if third_byte[i] != 0 {
            byte |= 1 << (7 - i);
        }
    }

    let mut fix_header = vec![0; length];

    fix_header[0] = 0 as u8;
    fix_header[1] = length as u8;
    fix_header[2] = byte as u8;

    header.fix_header = fix_header;

    header.header_length = length;
    header.third_byte = byte as usize;

    return header;
}



/// Writes the bytes of a value into the header vector at the specified positions.
///
/// # Arguments
///
/// * `header` - The header vector to write into.
/// * `value` - The value to write.
/// * `start_pos` - The starting position in the header vector.
/// * `end_pos` - The ending position in the header vector.
///
pub fn write_in_header(header: &mut Vec<u8>, value: u64, start_pos: usize, end_pos: usize) {
    let value_bytes = value.to_be_bytes();

    let byte_count = end_pos - start_pos + 1;

    for i in 0..byte_count {
        header[end_pos - i] = value_bytes[value_bytes.len() - 1 - i];
    }
}



/// Writes a hex string value into the header vector at the specified positions.
///
/// # Arguments
///
/// * `header` - The header vector to write into.
/// * `value` - The hex string value to write.
/// * `start_pos` - The starting position in the header vector.
/// * `end_pos` - The ending position in the header vector.
///
/// # Returns
///
/// Returns `Result<(), RError>` indicating success or an error.
///
pub fn write_hex_in_header(
    header: &mut Vec<u8>,
    value: &str,
    start_pos: usize,
    end_pos: usize,
) -> Result<(), RError> {
    if end_pos - start_pos + 1 < value.len() / 2 {
        return Err(RError::new(
            RErrorKind::InputOutputError,
            "Can't write string in byte-vector. Reason: vector to short.",
        ));
    }

    let mut bytes = Vec::new();
    for i in (0..value.len()).step_by(2) {
        let byte = u8::from_str_radix(&value[i..i + 2], 16)
            .map_err(|_| RError::new(RErrorKind::ConvertionError, "Failed to parse hex string."))?;
        bytes.push(byte);
    }

    for i in 0..bytes.len() {
        header[start_pos + i] = bytes[i];
    }

    return Ok(());
}



/// Separates the header and data portions from a vector of bytes.
///
/// # Arguments
///
/// * `data` - The vector of bytes containing the header and data.
///
/// # Returns
///
/// Returns `Result<(Vec<u8>, Vec<u8>), RError>` containing the separated header and data vectors, or an error.
///
pub fn separate_header(data: &Vec<u8>) -> Result<(Vec<u8>, Vec<u8>), RError> {
    let first_byte = data[0];

    if first_byte != 0 {
        return Err(RError::new(
            RErrorKind::InputOutputError,
            "Header does not comply with the guidelines and cannot be read.",
        ));
    }

    let second_byte = data[1];

    let (header, data) = data.split_at(second_byte as usize);

    let header = header.to_vec();
    let data = data.to_vec();

    return Ok((header, data));
}


/// Reads a header from a byte vector, extracts the header data, and returns it.
///
/// # Arguments
///
/// * `byte_vec` - The byte vector containing the header.
///
/// # Returns
///
/// Returns `Result<HeaderData, RError>` containing the extracted header data, or an error.
///
pub fn read_send_header(byte_vec: &Vec<u8>) -> Result<HeaderData, RError> {
    let new_header = read_header(&byte_vec)?;
    let header_data = extract_header_data(&new_header);

    return header_data;
}



/// Checks the integrity of a chunk by comparing its calculated hash with the hash stored in the header.
///
/// # Arguments
///
/// * `header_hash` - The hash stored in the header.
/// * `calc_hash_alg` - The hash algorithm used to calculate the hash of the chunk.
/// * `byte_vec` - The byte vector representing the chunk.
///
/// # Returns
///
/// Returns a boolean indicating whether the chunk hash matches the header hash.
///
pub fn check_chunk_hash(
    header_hash: &Option<String>,
    calc_hash_alg: &Option<Hash>,
    byte_vec: &Vec<u8>,
) -> bool {
    if let Some(hash_alg) = calc_hash_alg {
        let hash = get_hash(byte_vec, &hash_alg);

        if let Some(h_hash) = header_hash {
            return h_hash.to_lowercase() == hash.to_lowercase();
        }
    }
    true
}



/// Writes a log entry to a specified file.
///
/// # Arguments
///
/// * `path` - The path to the log file.
/// * `user_hash` - The user hash.
/// * `parent_hash_alg` - The hash algorithm used for the parent hash.
/// * `part_num` - The part number.
/// * `max_num` - The maximum number of parts.
/// * `part_size` - The size of the part.
/// * `parent_hash` - The parent hash.
/// * `part_hash_alg` - The hash algorithm used for the part hash (optional).
/// * `part_hash` - The part hash (optional).
///
/// # Returns
///
/// Returns the path to the log file if successful.
///
pub fn write_to_log_file(
    path: &str,
    user_hash: &str,
    parent_hash_alg: &Hash,
    part_num: u64,
    max_num: u64,
    part_size: u64,
    parent_hash: &str,
    part_hash_alg: &Option<Hash>,
    part_hash: &Option<String>,
) -> Result<String, Error> {
    let time = Utc::now().format("%d.%m.%Y - %H:%M:%S%.3f").to_string();

    let opt_hash = match (part_hash_alg, part_hash) {
        (Some(alg), Some(hash)) => {
            format!(" - [{}] - [{}]", alg.to_string(), hash)
        }
        _ => "".to_string(),
    };

    let log_line = format!(
        "[{}] - [{}] - [{}] - [{}] - [{}] - [{}] - [{} bytes]{}",
        time,
        user_hash,
        parent_hash_alg.to_string(),
        parent_hash,
        part_num,
        max_num,
        part_size,
        opt_hash
    );

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{}", log_line).unwrap();

    Ok(path.to_string())
}



/// Validates the log entries in the given vector and identifies any missing chunk parts.
///
/// # Arguments
///
/// * `vec` - A vector of log entries.
///
/// # Returns
///
/// Returns a tuple indicating the range of missing chunk parts. The first value represents the lowest missing chunk part,
/// and the second value represents the highest missing chunk part. If no missing chunk parts are found, both values will be zero.
///
pub fn validate_log_file(vec: &Vec<LogEntry>) -> (u64, u64) {
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

    match missing_vec.len() {
        0 => return (0, 0),
        1 => return (missing_vec[0], missing_vec[0]),
        _ => {
            let x = missing_vec.len() - 1;
            return (missing_vec[0], missing_vec[x]);
        }
    }
}



/// Reads the stop signal from the byte vector and extracts the hash value.
///
/// # Arguments
///
/// * `byte_vec` - The byte vector containing the stop signal.
///
/// # Returns
///
/// Returns the hash value extracted from the stop signal if successful.
///
/// # Errors
///
/// The function can return an error if there is an issue with the byte vector or if the stop signal cannot be read or parsed.
///
pub fn read_stop(byte_vec: &Vec<u8>) -> Result<String, RError> {
    let stop = String::from_utf8_lossy(&byte_vec).into_owned();

    let regex = Regex::new(STOP_REGEX)
        .map_err(|err| RError::new(RErrorKind::RegexError, &err.to_string()))?;

    if let Some(captures) = regex.captures(&stop) {
        let hash = captures.get(1).map_or("", |m| m.as_str()).to_string();

        return Ok(hash);
    }

    return Err(RError::new(
        RErrorKind::InputOutputError,
        "Can't read StopSignal.",
    ));
}


/// Creates a stop signal byte vector with the specified hash value.
///
/// # Arguments
///
/// * `hash` - The hash value to include in the stop signal.
///
/// # Returns
///
/// Returns a byte vector representing the stop signal if successful.
///
/// # Errors
///
/// The function can return an error if there is an issue with writing the hash value to the byte vector.
///
pub fn create_stop(hash: &str) -> Result<Vec<u8>, Error> {
    let mut vec = Vec::new();
    vec.push(3);
    write!(vec, "[{}]", hash)?;

    return Ok(vec);
}



/// Reads a log file at the specified path and extracts log entries based on the provided regular expression.
///
/// # Arguments
///
/// * `path` - The path to the log file.
/// * `buffer_size` - The buffer size to use for reading the file.
/// * `regex` - The regular expression pattern to match log entries.
///
/// # Returns
///
/// Returns a vector of `LogEntry` structures representing the extracted log entries if successful.
///
/// # Errors
///
/// The function can return an error if the specified path is not a valid file, if there is an error opening the file, or if there is an issue with the regular expression pattern.
///
pub fn read_log_file(path: &str, buffer_size: usize, regex: &str) -> Result<Vec<LogEntry>, Error> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Path can not be opended.",
            ))
        }
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
                let chunk_hash_alg_tmp = match &captures[9] {
                    "SIPHASH24" => Hash::SIPHASH24,
                    "SHA512" => Hash::SHA512,
                    "MD5" => Hash::MD5,
                    "SHA256" => Hash::SHA256,
                    _ => {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Hash-Algorithm not found",
                        ))
                    }
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

            let file_hash_alg = match &captures[3] {
                "SIPHASH24" => Hash::SIPHASH24,
                "SHA512" => Hash::SHA512,
                "MD5" => Hash::MD5,
                "SHA256" => Hash::SHA256,
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Hash-Algorithm not found",
                    ))
                }
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





/// Validates the integrity of a file by reading the corresponding log file.
///
/// # Arguments
///
/// * `output_dir` - The output directory where the log file is located.
/// * `_file_hash` - The hash of the file (not currently used).
///
/// # Returns
///
/// The function returns a tuple containing the start position and end position of missing log entries if any are found.
///
/// # Errors
///
/// The function can return an error if there is an issue reading the log file.
pub fn validate_file(output_dir: &str, _file_hash: &str) -> Result<(u64, u64), RError> {


    let mut log_entry_vec = read_log_file(&output_dir, BUFFER_SIZE, LOGGER_REGEX)
        .map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

    let (startpos, endpos) = validate_log_file(&mut log_entry_vec);

    return Ok((startpos, endpos));
}

