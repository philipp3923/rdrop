use std::{
    collections::hash_map::DefaultHasher,
    fs::File,
    hash::Hasher,
    io::Error,
    io::{BufReader, Read, Seek, SeekFrom},
};

use md5::Md5;
use sha2::{Digest, Sha256, Sha512};

pub const BUFFER_HASH_SIZE: usize = 1024 * 1024 * 250;

//Enum with string-len of hash
#[derive(Debug)]
pub enum Hash {
    SIPHASH24 = 16,
    MD5 = 32,
    SHA256 = 64,
    SHA512 = 128,
}

impl Hash {
    pub fn to_string(&self) -> String {
        match &self {
            Hash::SIPHASH24 => "SIPHASH24".to_string(),
            Hash::MD5 => "MD5".to_string(),
            Hash::SHA256 => "SHA256".to_string(),
            Hash::SHA512 => "SHA512".to_string(),
        }
    }
}

pub fn get_hash_from_file(file: &File) -> Result<String, Error> {
    return get_file_hash(file, BUFFER_HASH_SIZE, &Hash::SIPHASH24, 0);
}

/// Calculates the hash value of a data vector using the specified hash algorithm.
///
/// # Arguments
///
/// * byte_vec - The data vector for which the hash value will be calculated.
/// * hash_algorithm - The hash algorithm to use for calculating the hash value.
///
/// # Returns
///
/// The function returns the hash value as a hexadecimal string.
/// 
pub fn get_hash(byte_vec: &Vec<u8>, hash_algorithm: &Hash) -> String {
    match hash_algorithm {
        Hash::SIPHASH24 => {
            let mut hasher = DefaultHasher::new();

            hasher.write(&byte_vec);
            let mut return_val = format!("{:x}", hasher.finish());

            if return_val.len() < 16 {
                return_val = format!("{:0<16}", return_val);
            }
            return return_val;
        }
        Hash::MD5 => {
            let mut hasher = Md5::new();

            hasher.update(&byte_vec);
            let return_val = format!("{:x}", hasher.finalize());
            return return_val;
        }
        Hash::SHA256 => {
            let mut hasher = Sha256::new();

            hasher.update(&byte_vec);
            let return_val = format!("{:x}", hasher.finalize());
            return return_val;
        }
        Hash::SHA512 => {
            let mut hasher = Sha512::new();

            hasher.update(&byte_vec);
            let return_val = format!("{:x}", hasher.finalize());
            return return_val;
        }
    }
}


/// Calculates the hash value of a file using the specified hash algorithm.
///
/// # Arguments
///
/// * file - The file for which the hash value will be calculated.
/// * buffer_size - The size of the buffer used for reading the file.
/// * file_hash - The hash algorithm to use for calculating the hash value.
/// * start_pos - The position within the file to start calculating the hash from.
///
/// # Returns
///
/// The function returns the hash value as a hexadecimal string.
///
/// # Errors
///
/// The function can return an error if there is an error reading the file or performing hash calculations. The Error type contains details about the error.
/// 
pub fn get_file_hash(
    file: &File,
    buffer_size: usize,
    file_hash: &Hash,
    start_pos: usize,
) -> Result<String, Error> {
    let mut buf_reader = BufReader::with_capacity(buffer_size, file);
    let mut buffer = vec![0; buffer_size];

    let length = start_pos.clone() as u64;

    //ignore parts of file if necessary, start calc hash after header,
    // deprecated, not in
    buf_reader.seek(SeekFrom::Start(length))?;

    // build hashes
    match file_hash {
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
        }
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
        }
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
        }
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
        }
    }
}



//hash-test-speed
//add file to test speed with
#[cfg(test)]
mod tests{

    use std::fs::File;
    use std::time::Instant;

    use crate::general::general::AppSettings;
    use crate::hash::hash::{Hash, get_file_hash};

    #[test]
    #[ignore]
    fn test_get_file_hash() {
        let file_path = "Testfile.pdf_part_000001.chunk";

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
            for (i, a) in alg.iter().enumerate() {
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
            println!("{}: {} sekunden", hash, avg_duration / 1000000);
        }
    }

}