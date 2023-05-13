use std::fs::File;
use std::io::{Error, ErrorKind, Write};

use regex::Regex;

use crate::error::error::{RError, RErrorKind};
use crate::general::general::{load_file_data, append_header, HeaderByte, BUFFER_SIZE};
use crate::hash::hash::{Hash, get_file_hash};

pub const OFFER_REGEX:&str = r"\[(.+)\] - \[(\d+)\] - \[(SHA256|SHA512|MD5|SIPHASH24)\] - \[([0-9a-zA-Z]+)\]";


#[derive(Debug)]
pub struct Offer{
    pub name:String,
    pub size:u64,
    pub hash_type:Hash,
    pub file_hash:String
}

impl Offer{
    pub fn new(name:&str, size:&str, hash_type:&str, file_hash:&str) -> Result<Self, Error>{

        let hash = match hash_type{
            "SIPHASH24" => Hash::SIPHASH24,
            "MD5" => Hash::MD5,
            "SHA256" => Hash::SHA256,
            "SHA512" => Hash::SHA512,
            _ => {
                return Err(Error::new(ErrorKind::InvalidInput, "Hash-Algorithm not found, bad input."));
            }
        };
        Ok(Self { name: name.to_string(), size: size.parse::<u64>().unwrap(), hash_type: hash, file_hash: file_hash.to_string() })
    }
}





// creates offer - byte-vector
// includes data to send to user
pub fn create_offer(buffer_size:usize, hash_alg:&Hash, file_path:&str) -> Result<Vec<u8>, Error>{

    //create file_data
    let file_data = load_file_data(file_path).unwrap();
    //load file
    let file = File::open(file_path)?;

    let is_dir = file.metadata()?.is_dir();
    if is_dir == true {
        //return error, only files are valid
        return Err(Error::new(ErrorKind::InvalidInput , "Path is a directory. Can only send Files (includes .zip)"));
    }

    //load hash
    let file_hash = get_file_hash(&file, buffer_size, &hash_alg, 0)?;
    
    let mut offer = Vec::new();

    write!(offer,"[{}] - [{}] - [{}] - [{}]", &file_data.name, &file_data.size, hash_alg.to_string(), &file_hash)?;

    return Ok(offer);
}



//read offer as string and returns offer struct
pub fn read_offer(offer_regex:&str, offer:&str) -> Result<Offer, RError>{

    let regex = Regex::new(offer_regex).map_err(|err| RError::new(RErrorKind::RegexError, &err.to_string()))?;

    if let Some(captures) = regex.captures(offer) {
        let name = captures.get(1).map_or("", |m| m.as_str());
        let size = captures.get(2).map_or("", |m| m.as_str());
        let hash_type = captures.get(3).map_or("", |m| m.as_str());
        let file_hash = captures.get(4).map_or("", |m| m.as_str());

        let offer = Offer::new(name, size, hash_type, file_hash).map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

        return Ok(offer);
    }

    return Err(RError::new(RErrorKind::InputOutputError, "Can't read Offer."));
}

// creates offer from file-path
pub fn create_offer_vec(file_hash:&Hash, path:&str) -> Result<Vec<u8>, Error>{

    let mut offer_byte_vec = create_offer(BUFFER_SIZE, &file_hash, &path)?;

    offer_byte_vec = append_header(offer_byte_vec, HeaderByte::SendOffer);

    return Ok(offer_byte_vec);
}

//read byte-vector and creates offer struct
pub fn read_offer_vec(byte_vec:&Vec<u8>) -> Result<Offer, RError>{

    let offer_as_string = String::from_utf8_lossy(&byte_vec).into_owned();
    let offer = read_offer(OFFER_REGEX, &offer_as_string)?;

    return Ok(offer);
}

