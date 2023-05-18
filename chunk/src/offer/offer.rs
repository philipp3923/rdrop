use std::fs::metadata;
use std::io::{Error, ErrorKind, Write};
use std::path::Path;

use regex::Regex;

use crate::error::error::{RError, RErrorKind};
use crate::general::general::{append_header, HeaderByte};
use crate::hash::hash::Hash;

pub const OFFER_REGEX: &str =
    r"\[(.+)\] - \[(\d+)\] - \[(SHA256|SHA512|MD5|SIPHASH24)\] - \[([0-9a-fA-F]+)\]";

#[derive(Debug)]
pub struct Offer {
    pub name: String,
    pub size: u64,
    pub hash_type: Hash,
    pub file_hash: String,
}

impl Offer {
    pub fn new(name: &str, size: &str, hash_type: &str, file_hash: &str) -> Result<Self, Error> {
        let hash = match hash_type {
            "SIPHASH24" => Hash::SIPHASH24,
            "MD5" => Hash::MD5,
            "SHA256" => Hash::SHA256,
            "SHA512" => Hash::SHA512,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Hash-Algorithm not found, bad input.",
                ));
            }
        };
        Ok(Self {
            name: name.to_string(),
            size: size.parse::<u64>().unwrap(),
            hash_type: hash,
            file_hash: file_hash.to_string(),
        })
    }
}

/// Creates an offer message as a byte vector.
///
/// # Arguments
///
/// * hash - The hash value of the file.
/// * size - The size of the file in bytes.
/// * path - The path of the file.
///
/// # Returns
///
/// The function returns a Result containing the offer message as a byte vector if successful.
///
/// # Errors
///
/// The function can return an error if the path does not point to a valid file or directory, or if there is an error while constructing the offer message.
/// The Error type contains details about the error.
/// 
pub fn create_offer_byte_msg(hash: &str, size: u64, path: &str) -> Result<Vec<u8>, Error> {
    let metadata = match metadata(path) {
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

    let name = Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut offer = Vec::new();

    write!(
        offer,
        "[{}] - [{}] - [{}] - [{}]",
        name,
        size,
        Hash::SIPHASH24.to_string(),
        &hash
    )?;

    offer = append_header(offer, HeaderByte::SendOffer);

    return Ok(offer);
}




/// Reads and parses an offer message.
///
/// # Arguments
///
/// * offer_regex - The regular expression pattern to match against the offer message.
/// * offer - The offer message string.
///
/// # Returns
///
/// The function returns a Result containing the parsed offer if successful.
///
/// # Errors
///
/// The function can return an error if the offer message does not match the provided regular expression pattern or if there is an error while parsing the offer.
/// The RError type contains details about the error.
/// 
pub fn read_offer(offer_regex: &str, offer: &str) -> Result<Offer, RError> {
    let regex = Regex::new(offer_regex)
        .map_err(|err| RError::new(RErrorKind::RegexError, &err.to_string()))?;

    if let Some(captures) = regex.captures(offer) {
        let name = captures.get(1).map_or("", |m| m.as_str());
        let size = captures.get(2).map_or("", |m| m.as_str());
        let hash_type = captures.get(3).map_or("", |m| m.as_str());
        let file_hash = captures.get(4).map_or("", |m| m.as_str());

        let offer = Offer::new(name, size, hash_type, file_hash)
            .map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

        return Ok(offer);
    }

    return Err(RError::new(
        RErrorKind::InputOutputError,
        "Can't read Offer.",
    ));
}



/// Reads and parses an offer message from a byte vector.
///
/// # Arguments
///
/// * byte_vec - The byte vector containing the offer message.
///
/// # Returns
///
/// The function returns a Result containing the parsed offer if successful.
///
/// # Errors
///
/// The function can return an error if the byte vector cannot be converted to a valid UTF-8 string or if there is an error while parsing the offer.
/// The RError type contains details about the error.
/// 
pub fn read_offer_vec(byte_vec: &Vec<u8>) -> Result<Offer, RError> {
    let offer_as_string = String::from_utf8_lossy(&byte_vec).into_owned();
    let offer = read_offer(OFFER_REGEX, &offer_as_string)?;

    return Ok(offer);
}

