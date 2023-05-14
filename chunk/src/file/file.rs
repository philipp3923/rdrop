//split file function

use std::{io::{BufReader, SeekFrom, Seek, Read, Error, Write}, fs::{File, create_dir_all, OpenOptions, metadata}, path::{Path}};

use crate::{hash::hash::Hash, general::general::{separate_header, read_send_header, check_chunk_hash, write_to_log_file, get_filename_from_dir, HeaderData, BUFFER_SIZE, CHUNK_SIZE, calc_chunk_count, CHUNK_HASH_TYPE, create_header, USER_HASH}};
use crate::{error::error::{RError, RErrorKind}, hash::hash::get_hash, general::general::{write_hex_in_header, write_in_header, Header}};




//wrapper for writing data in vector
pub fn write_data_vec(header_data:&HeaderData, data_vector:&Vec<u8>, output_path:&str) -> Result<String, Error>{

    let mut outpath:String = "".to_string();
    if check_chunk_hash(&header_data.chunk_hash, &header_data.chunk_hash_alg, &data_vector) == true{
        outpath = merge_file_on_path(&output_path, &data_vector, header_data.chunk_pos, CHUNK_SIZE)?;
        let logfile_path = format!("{}.rdroplog", output_path);
        let _ = write_to_log_file(&logfile_path, &header_data.user_hash, &header_data.file_hash_alg, header_data.chunk_pos, header_data.chunk_max, header_data.chunk_length as u64, &header_data.file_hash, &header_data.chunk_hash_alg, &header_data.chunk_hash)?;
        return Ok(outpath)
    }

    return Err(Error::new(std::io::ErrorKind::InvalidData, "Corrupted Data, can't verify hash"));

}





// splits a file and returns ordered part
pub fn split_file_single(buf_reader:&mut BufReader<&mut File>, part_num:usize, file_size:usize, reg_chunk_size:usize, file_hash:&str, chunk_count_max:u64, user_hash:&str, header:&mut Header, chunk_hash:&Option<Hash>) -> Result<Vec<u8>, RError>{

    let mut chunk_size = reg_chunk_size;

    if part_num as u64 == chunk_count_max{

        chunk_size = file_size - (chunk_size * (part_num - 1));
    }

    let mut start_pos:u64;
    
    if part_num == 1 {
        start_pos = 0;
    }
    else {
        start_pos = (part_num as u64 - 1) * reg_chunk_size as u64;
    }
    let mut buffer = vec![0;chunk_size];

    buf_reader.seek(SeekFrom::Start(start_pos)).map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

    buf_reader.read(&mut buffer).map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

    write_in_header(&mut header.fix_header,     part_num as u64,    header.chunk_pos_s,         header.chunk_pos_e);
    write_in_header(&mut header.fix_header,     chunk_count_max,    header.chunk_max_pos_s,     header.chunk_max_pos_e);
    write_in_header(&mut header.fix_header,     chunk_size as u64,  header.chunk_length_pos_s,  header.chunk_length_pos_e);
    write_hex_in_header(&mut header.fix_header, user_hash,          header.user_pos_s,          header.user_pos_e)?;
    write_hex_in_header(&mut header.fix_header, file_hash,          header.file_hash_pos_s,     header.file_hash_pos_e)?;

    if chunk_hash.is_some(){
        let chunk_hash = get_hash(&buffer, &chunk_hash.as_ref().expect("chunk_hash not set."));
        write_hex_in_header(&mut header.fix_header, &chunk_hash, header.chunk_hash_pos_s.expect("chunk_hash not set."), header.chunk_hash_pos_e.expect("chunk_hash not set."))?;
    }

    let mut byte_vec:Vec<u8> = Vec::new();
    byte_vec.extend(header.fix_header.iter());
    byte_vec.extend(buffer.iter());

return Ok(byte_vec);
}


// writes byte-vec in file
pub fn merge_file_on_path(output_path:&str, byte_vec:&Vec<u8>, chunk_number:u64, reg_chunk_size:usize) -> Result<String, Error>{
    
    println!("1");
    
    //create output file in outputDir if not there
    let mut output_file = match OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&output_path){
            Ok(file) => file,
            Err(err) => {
                return Err(err);
            }      
    };

    println!("2");
    
    //get start_pos
    let start_pos = reg_chunk_size as u64 * (chunk_number - 1);

    //check file_size
    let output_file_size = output_file.metadata().unwrap().len();

    println!("3");
    
    if output_file_size < start_pos{

        output_file.seek(SeekFrom::End(0)).unwrap();

        let size_to_append = start_pos - output_file_size;

        let zero_byte_vec:[u8;256000] = [0;256000];

        for _ in 0..(size_to_append / zero_byte_vec.len() as u64){
            output_file.write_all(&zero_byte_vec).unwrap();
        }

        let size_modulo = size_to_append % zero_byte_vec.len() as u64;

        if size_modulo != 0 {
            output_file.write_all(&zero_byte_vec[0..size_modulo as usize]).unwrap();
        }
    }
                                    
    println!("4");

    output_file.seek(SeekFrom::Start(start_pos)).unwrap();
    output_file.write_all(byte_vec).unwrap();
    
    
    return Ok(output_path.to_string());
}

// writes byte-vec in file
pub fn merge_file(output_path:&str, file_name:&str, byte_vec:&Vec<u8>, chunk_number:u64, reg_chunk_size:usize) -> Result<String, Error>{
    
    let result_path = format!("{}/{}",&output_path, &file_name);
    let result_path = Path::new(&result_path);

    if let Some(parent) = result_path.parent() {
        create_dir_all(parent)?;
    }
    //create output file in outputDir if not there
    let mut output_file = match OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&result_path){
            Ok(file) => file,
            Err(err) => {
                return Err(err);
            }      
    };

    //get start_pos
    let start_pos = reg_chunk_size as u64 * (chunk_number - 1);

    //check file_size
    let output_file_size = output_file.metadata().unwrap().len();

    if output_file_size < start_pos{

        output_file.seek(SeekFrom::End(0)).unwrap();

        let size_to_append = start_pos - output_file_size;

        let zero_byte_vec:[u8;256000] = [0;256000];

        for _ in 0..(size_to_append / zero_byte_vec.len() as u64){
            output_file.write_all(&zero_byte_vec).unwrap();
        }

        let size_modulo = size_to_append % zero_byte_vec.len() as u64;

        if size_modulo != 0 {
            output_file.write_all(&zero_byte_vec[0..size_modulo as usize]).unwrap();
        }
    }

    output_file.seek(SeekFrom::Start(start_pos)).unwrap();
    output_file.write_all(byte_vec).unwrap();
    
    
    return Ok(result_path.to_str().to_owned().unwrap().to_string());
}


//wrapper for writing data in vector
pub fn r_w_data_vec(byte_vec:&Vec<u8>, output_dir:&str) -> Result<HeaderData, RError>{

    let (header_vector, data_vector) = separate_header(&byte_vec)?;
    let header_data =  read_send_header(&header_vector)?;

    let outpath = format!("{}/{}",&output_dir, &header_data.file_hash);
    let file_name = get_filename_from_dir(&outpath)?;


    let mut output_path = "".to_string();

    if check_chunk_hash(&header_data.chunk_hash, &header_data.chunk_hash_alg, &data_vector) == true{
        output_path = merge_file(&outpath, &file_name, &data_vector, header_data.chunk_pos, CHUNK_SIZE).map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;
    }

    let logfile_path = format!("{}.rdroplog", output_path);
    let _log_written = write_to_log_file(&logfile_path, &header_data.user_hash, &header_data.file_hash_alg, header_data.chunk_pos, header_data.chunk_max, header_data.chunk_length as u64, &header_data.file_hash, &header_data.chunk_hash_alg, &header_data.chunk_hash).map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

    return Ok(header_data)

}


// create data byte-vector
pub fn create_data_vec(path:&str, chunk_num:u64, file_hash:&str) -> Result<Vec<u8>, RError>{

    let mut file = File::open(path).map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;
    let file_size = metadata(path).map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?.len();
    let mut buf_reader = BufReader::with_capacity(BUFFER_SIZE, &mut file);
    let max_chunk_count = calc_chunk_count(CHUNK_SIZE, file_size)?;

    let mut header = create_header(file_size, max_chunk_count, &Hash::SIPHASH24, &Some(CHUNK_HASH_TYPE));
    let split_vec = split_file_single(&mut buf_reader, chunk_num as usize, file_size as usize, CHUNK_SIZE, &file_hash, max_chunk_count, &USER_HASH, &mut header, &Some(CHUNK_HASH_TYPE))?;

    return Ok(split_vec);
}
