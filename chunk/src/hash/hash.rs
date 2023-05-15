use std::{fs::{File}, io::{BufReader, Read, SeekFrom, Seek}, io::{Error}, collections::hash_map::DefaultHasher, hash::Hasher};
use std::time::Instant;

use sha2::{Sha256, Sha512, Digest};
use md5::Md5;

use crate::general::general::{AppSettings, BUFFER_SIZE};

pub const BUFFER_HASH_SIZE:usize = 1024 * 1024*250;


//Enum with string-len of hash
#[derive(Debug)]
pub enum Hash{
    SIPHASH24 = 16,
    MD5 = 32,
    SHA256 = 64,
    SHA512 = 128
}

impl Hash{
    pub fn to_string(&self) -> String {
        match &self {
            Hash::SIPHASH24 => "SIPHASH24".to_string(),
            Hash::MD5 => "MD5".to_string(),
            Hash::SHA256 => "SHA256".to_string(),
            Hash::SHA512 => "SHA512".to_string(),
        }
    }
}


pub fn get_hash_from_file(file:&File) -> Result<String, Error>{

    return get_file_hash(file, BUFFER_HASH_SIZE, &Hash::SIPHASH24, 0);
}


/*
    calc hash of byte-vector
*/
pub fn get_hash(byte_vec:&Vec<u8>, hash_algorithm:&Hash) -> String{

    match hash_algorithm{
        Hash::SIPHASH24 => {
            let mut hasher = DefaultHasher::new();

            hasher.write(&byte_vec);
            let mut return_val = format!("{:x}", hasher.finish());

            if return_val.len() < 16 {
                return_val = format!("{:0<16}", return_val);
            }
            return return_val;
        },
        Hash::MD5 => {
            let mut hasher = Md5::new();

            hasher.update(&byte_vec);          
            let return_val = format!("{:x}", hasher.finalize());
            return return_val;
        },
        Hash::SHA256 => {
            let mut hasher = Sha256::new();

            hasher.update(&byte_vec);
            let return_val = format!("{:x}", hasher.finalize());
            return return_val;
        },
        Hash::SHA512 => {
            let mut hasher = Sha512::new();

            hasher.update(&byte_vec);            
            let return_val = format!("{:x}", hasher.finalize());
            return return_val;
        }
    }
}



// generates hash for a file with choosen hash-algorithm
/*
    limits space-use of RAM with chosen buffer-size
*/
pub fn get_file_hash(file:&File, buffer_size:usize, file_hash: &Hash, start_pos:usize) -> Result<String, Error>{

    let mut buf_reader = BufReader::with_capacity(buffer_size, file);
    let mut buffer = vec![0;buffer_size];

    let length = start_pos.clone() as u64;

    //ignore parts of file if necessary, start calc hash after header,
    // deprecated, not in
    buf_reader.seek(SeekFrom::Start(length))?;


    // build hashes
    match file_hash{
        Hash::SIPHASH24 => {
            let mut hasher = DefaultHasher::new();

            loop {
                let bytes = buf_reader.read(&mut buffer)?;
                if bytes == 0 {
                    break;
                }
                hasher.write(&buffer[0..bytes]);
            }
            let mut return_val = format!("{:x}", hasher.finish());

            if return_val.len() < 16 {
                return_val = format!("{:0<16}", return_val);
            }

            return Ok(return_val);
        },
        Hash::MD5 => {
            let mut hasher = Md5::new();

            loop {
                let bytes = buf_reader.read(&mut buffer)?;
                if bytes == 0 {
                    break;
                }
                hasher.update(&buffer[0..bytes]);
            }
            
            let return_val = format!("{:x}", hasher.finalize());
            return Ok(return_val);
        },
        Hash::SHA256 => {
            let mut hasher = Sha256::new();

            loop {
                let bytes = buf_reader.read(&mut buffer)?;
                if bytes == 0 {
                    break;
                }
                hasher.update(&buffer[0..bytes]);
            }
            
            let return_val = format!("{:x}", hasher.finalize());
            return Ok(return_val);
        },
        Hash::SHA512 => {
            let mut hasher = Sha512::new();

            loop {
                let bytes = buf_reader.read(&mut buffer)?;
                if bytes == 0 {
                    break;
                }
                hasher.update(&buffer[0..bytes]);
            }
            
            let return_val = format!("{:x}", hasher.finalize());
            return Ok(return_val);
        },
   }
}

#[test]
fn test_get_file_hash(){

    let file_path = "Testfile.pdf_part_000001.chunk";
    //let file = File::open(file_path).unwrap();
    
    let app_settings = AppSettings::default();

    let mut alg = Vec::new();
    alg.push(Hash::SHA512);
    alg.push(Hash::SHA256);
    alg.push(Hash::MD5);
    alg.push(Hash::SIPHASH24);

    let header_length = 0;

    let mut total_durations = Vec::new();
    total_durations.push(0);
    total_durations.push(0);
    total_durations.push(0);
    total_durations.push(0);

    let x = 3;

    for _ in 0..x {

        for (i,a) in alg.iter().enumerate() {

            let file = File::open(file_path).unwrap();

            let hash = match a {
                &Hash::MD5 => Hash::MD5,
                &Hash::SIPHASH24 => Hash::SIPHASH24,
                &Hash::SHA256 => Hash::SHA256,
                &Hash::SHA512 => Hash::SHA512,
            };

            let start_time = Instant::now();
            let result = get_file_hash(&file, app_settings.buffer_size, &hash, header_length);
            let duration = start_time.elapsed();
            
            assert!(result.is_ok());
            println!("{}", result.unwrap());


            total_durations[i] += duration.as_micros();
        }
    }

    for (i, algorithm) in alg.iter().enumerate() {
        let avg_duration = total_durations[i] / x;

        let hash = match algorithm {
            &Hash::MD5 => "Hash::MD5",
            &Hash::SIPHASH24 => "Hash::SIPHASH24",
            &Hash::SHA256 => "Hash::SHA256",
            &Hash::SHA512 => "Hash::SHA512",
        };

        println!("{}: {} microseconds", hash, avg_duration);
        println!("{}: {} sekunden", hash, avg_duration/1000000);
    }
}