use std::{
    io::{Error, ErrorKind, Write},
};

use regex::Regex;

use crate::{
    error::error::{RError, RErrorKind},
    general::general::{
        append_header, HeaderByte,
        CHUNK_HASH_TYPE, CHUNK_SIZE,
    },
    hash::hash::Hash,

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


/// Creates an order byte vector for sending order information.
///
/// # Arguments
///
/// * start - The starting position of the ordered chunk.
/// * end - The ending position of the ordered chunk.
/// * file_hash - The hash of the file.
///
/// # Returns
///
/// The function returns a Result containing the order byte vector if successful.
///
/// # Errors
///
/// The function can return an error if there is an error while creating the order or appending the header. The Error type contains details about the error.
/// 
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

/// Reads an order from a byte vector.
///
/// # Arguments
///
/// * byte_vec - The byte vector containing the order information.
///
/// # Returns
///
/// The function returns a Result with the parsed Order if successful.
///
/// # Errors
///
/// The function can return an error if there is an error while parsing the order or applying regular expressions. The RError type contains details about the error.
/// 
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



/// Creates an order in the form of a byte vector.
///
/// # Arguments
///
/// * start_pos - The starting position of the order.
/// * end_pos - The ending position of the order.
/// * chunk_size - The size of each chunk.
/// * file_hash_alg - The hash algorithm used for file hashing.
/// * file_hash - The hash value of the file.
/// * file_name - The name of the file.
/// * _chunk_hash_type - (Optional) The hash algorithm used for chunk hashing. This argument is currently unused.
///
/// # Returns
///
/// The function returns a Result with the byte vector representing the order if successful.
///
/// # Errors
///
/// The function can return an error if there is an error while formatting the order or writing to the byte vector.
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

