use std::io::Error;

pub mod hash;
pub mod general;
mod time;
mod error;
pub mod offer;
pub mod order;
pub mod file;

use error::error::RError;
use general::general::{HeaderData, HeaderByte, FileData, load_file_data};
use offer::offer::{Offer, create_offer_byte_msg, read_offer_vec};
use order::order::{create_order_from_offer, Order, create_order_byte_vec};

use crate::general::general::{validate_file, CHUNK_SIZE, CHUNK_HASH_TYPE, BUFFER_SIZE, get_file_data, get_chunk_count,separate_header, read_send_header};
use crate::hash::hash::{ Hash, get_hash_from_file};
use crate::order::order::{read_order, create_order_from_logfile};
use crate::file::file::{r_w_data_vec, create_data_vec, write_data_vec};






pub fn read_offer_byte_vec(byte_vec:&Vec<u8>) -> Result<Offer, RError>{
    return read_offer_vec(byte_vec);
}

pub fn create_order_from_offer_byte_vec(file_hash_alg:&Hash, output_dir:&str, offer:&Offer) -> Result<Vec<u8>, RError>{
    return create_order_from_offer(CHUNK_SIZE, file_hash_alg, &Some(CHUNK_HASH_TYPE), output_dir, offer);
}

pub fn read_order_byte_vec(byte_vec:&mut Vec<u8>) -> Result<Order, RError>{
    return read_order(byte_vec);
}
/*
pub fn create_data_byte_vec(path:&str, order:&Order, chunk_num:usize) -> Result<Vec<u8>, RError>{
    return create_data_vec(path, order, chunk_num);
}
 */

pub fn write_data_byte_vec(byte_vec:&Vec<u8>, output_dir:&str) -> Result<HeaderData, RError>{
    return r_w_data_vec(byte_vec, output_dir);
}

pub fn valid_file(output_dir:&str, file_hash:&str) -> Result<(u64, u64),RError>{
    return validate_file(output_dir, file_hash);
}

pub fn create_order_byte_vec_from_logfile(path:&str) -> Result<Vec<u8>, Error>{
    return create_order_from_logfile(path,BUFFER_SIZE, CHUNK_SIZE);
}

