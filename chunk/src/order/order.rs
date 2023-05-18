use std::{
    fs::{create_dir_all, File},
    io::{Error, ErrorKind, Write},
    path::PathBuf,
};

use regex::Regex;

use crate::{
    error::error::{RError, RErrorKind},
    general::general::{
        append_header, calc_chunk_count, read_log_file, validate_log_file, HeaderByte,
        CHUNK_HASH_TYPE, CHUNK_SIZE, LOGGER_REGEX,
    },
    hash::hash::Hash,
    offer::offer::Offer,
};

pub const ORDER_REGEX: &str = r"\[(\d+)\]\s-\s\[(SHA256|SHA512|MD5|SIPHASH24)\]\s-\s\[([a-fA-F0-9]+)\]\s-\s\[(.*)\]\s-\s\[(\d+)\]\s-\s\[(\d+)\](\s-\s\[(SHA256|SHA512|MD5|SIPHASH24)\])?";

#[derive(Debug)]
pub struct Order {
    pub chunk_size: usize,
    pub file_hash_type: Hash,
    pub file_hash: String,
    pub file_name: String,
    pub start_num: u64,
    pub end_num: u64,
}

impl Order {
    pub fn new(
        chunk_size: &str,
        file_hash_type: &str,
        file_hash: &str,
        file_name: &str,
        start_num: &str,
        end_num: &str,
    ) -> Result<Self, Error> {
        let file_hash_type = match file_hash_type {
            "SIPHASH24" => Hash::SIPHASH24,
            "MD5" => Hash::MD5,
            "SHA256" => Hash::SHA256,
            "SHA512" => Hash::SHA512,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Hash-Alforithm not implemented.",
                ));
            }
        };

        let file_hash = file_hash.to_string();
        let file_name = file_name.to_string();
        let chunk_size: usize = chunk_size.parse::<usize>().unwrap();
        let start_num: u64 = start_num.parse::<u64>().unwrap();
        let end_num: u64 = end_num.parse::<u64>().unwrap();

        Ok(Self {
            chunk_size,
            file_hash_type,
            file_hash,
            file_name,
            start_num,
            end_num,
        })
    }
}

pub fn create_order_byte_vec(start: u64, end: u64, file_hash: &str) -> Result<Vec<u8>, Error> {
    let mut order_byte_vec = create_order(
        start,
        end,
        CHUNK_SIZE,
        &Hash::SIPHASH24,
        file_hash,
        "",
        &Some(CHUNK_HASH_TYPE),
    )?;

    order_byte_vec = append_header(order_byte_vec, HeaderByte::SendOrder);

    return Ok(order_byte_vec);
}

//read_order
pub fn read_order(byte_vec: &mut Vec<u8>) -> Result<Order, RError> {
    //removes first entry
    byte_vec.remove(0);
    let order = String::from_utf8_lossy(&byte_vec).as_ref().to_string();

    let regex = Regex::new(ORDER_REGEX)
        .map_err(|err| RError::new(RErrorKind::RegexError, &err.to_string()))?;

    if let Some(captures) = regex.captures(&order) {
        let chunk_size = captures.get(1).map_or("", |m| m.as_str());
        let file_hash_type = captures.get(2).map_or("", |m| m.as_str());
        let file_hash = captures.get(3).map_or("", |m| m.as_str());
        let file_name = captures.get(4).map_or("", |m| m.as_str());
        let start_number = captures.get(5).map_or("", |m| m.as_str());
        let end_number = captures.get(6).map_or("", |m| m.as_str());

        let order = Order::new(
            chunk_size,
            file_hash_type,
            file_hash,
            file_name,
            start_number,
            end_number,
        )
        .map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()));

        return order;
    }
    return Err(RError::new(
        RErrorKind::InputOutputError,
        "Can't read Order.",
    ));
}

//create_order
pub fn create_order(
    start_pos: u64,
    end_pos: u64,
    chunk_size: usize,
    file_hash_alg: &Hash,
    file_hash: &str,
    file_name: &str,
    _chunk_hash_type: &Option<Hash>,
) -> Result<Vec<u8>, Error> {
    let mut byte_vec = Vec::new();
    let order_str = format!(
        "[{}] - [{}] - [{}] - [{}] - [{}] - [{}]",
        chunk_size,
        file_hash_alg.to_string(),
        file_hash,
        file_name,
        start_pos,
        end_pos
    );

    write!(byte_vec, "{}", order_str)?;
    return Ok(byte_vec);
}

//creates file and logfile in new dir (filehash)
pub fn create_file_with_order(
    parent_path: &str,
    parent_hash: &str,
    file_name: &str,
) -> Result<String, Error> {
    let dir_path = format!("{}/{}", &parent_path, &parent_hash);
    create_dir_all(&dir_path)?;

    let log_path = format!("{}/{}.rdroplog", dir_path, file_name);

    if !File::open(&log_path).is_ok() {
        File::create(&log_path)?;
    }

    let file_path = format!("{}/{}", dir_path, file_name);

    if !File::open(&file_path).is_ok() {
        File::create(&file_path)?;
    }

    Ok(file_path)
}

//reads offer data and creates an order
pub fn create_order_from_offer(
    chunk_size: usize,
    file_hash_alg: &Hash,
    chunk_hash_type: &Option<Hash>,
    output_dir: &str,
    offer: &Offer,
) -> Result<Vec<u8>, RError> {
    let max_count = calc_chunk_count(chunk_size, offer.size)?;

    let mut order_byte_vec = create_order(
        1,
        max_count,
        chunk_size,
        &file_hash_alg,
        &offer.file_hash,
        &offer.name,
        &chunk_hash_type,
    )
    .map_err(|err| {
        RError::new(
            RErrorKind::ConvertionError,
            &format!("Can't create offer-byte-vector. Error: {}", err),
        )
    })?;

    order_byte_vec = append_header(order_byte_vec, HeaderByte::SendOrder);

    let _file_name = create_file_with_order(&output_dir, &offer.file_hash, &offer.name)
        .map_err(|err| RError::new(RErrorKind::RegexError, &err.to_string()))?;

    return Ok(order_byte_vec);
}

//reads and validates logfile and create order
pub fn create_order_from_logfile(
    path: &str,
    buffer_size: usize,
    chunk_size: usize,
) -> Result<Vec<u8>, Error> {
    let vec = read_log_file(path, buffer_size, LOGGER_REGEX)?;

    let (start_pos, end_pos) = validate_log_file(&vec);

    let file_hash_alg = &vec[0].file_hash_alg;
    let file_hash = &vec[0].file_hash;
    let chunk_hash_type = &vec[0].chunk_hash_alg;

    let mut file_name = PathBuf::from(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    if file_name.ends_with(".rdroplog") {
        file_name.truncate(file_name.len() - 4);
    }

    let byte_vec = create_order(
        start_pos,
        end_pos,
        chunk_size,
        file_hash_alg,
        file_hash,
        &file_name,
        chunk_hash_type,
    )?;

    return Ok(byte_vec);
}
