use std::{
    fs::{metadata, File, OpenOptions},
    io::{BufReader, Error, Read, Seek, SeekFrom, Write}
};

use crate::{
    error::error::{RError, RErrorKind},
    general::general::{write_hex_in_header, write_in_header, Header},
    hash::hash::get_hash,
};
use crate::{
    general::general::{
        calc_chunk_count, check_chunk_hash, create_header,
        write_to_log_file, HeaderData, BUFFER_SIZE, CHUNK_HASH_TYPE, CHUNK_SIZE,
        USER_HASH,
    },
    hash::hash::Hash,
};


/// Writes a data vector to a file.
///
/// # Arguments
/// 
/// * header_data - The header data containing information about the file.
/// * data_vector - The vector of data to be written to the file.
/// * output_path - The path where the file will be written.
///
/// # Returns
///
/// The function returns a Result with two possible outcomes:
/// * Ok(log_path) - If the data vector is successfully written to the file. log_path is the path of the log file generated during the write operation.
/// * Err(error) - If there is an error during the write operation, including cases where the data is corrupted and the hash cannot be verified.
/// 
/// # Errors
///
/// The function can return an error if the data vector is corrupted and the hash cannot be verified. The Error type contains details about the error.
/// 
pub fn write_data_vec(
    header_data: &HeaderData,
    data_vector: &Vec<u8>,
    output_path: &str,
) -> Result<String, Error> {

    if check_chunk_hash(
        &header_data.chunk_hash,
        &header_data.chunk_hash_alg,
        &data_vector,
    ) == true
    {
        _ = merge_file_on_path(
            &output_path,
            &data_vector,
            header_data.chunk_pos,
            CHUNK_SIZE,
        )?;
        let logfile_path = format!("{}.rdroplog", output_path);
        let _log_path = write_to_log_file(
            &logfile_path,
            &header_data.user_hash,
            &header_data.file_hash_alg,
            header_data.chunk_pos,
            header_data.chunk_max,
            header_data.chunk_length as u64,
            &header_data.file_hash,
            &header_data.chunk_hash_alg,
            &header_data.chunk_hash,
        )?;
        return Ok(_log_path);
    }

    return Err(Error::new(
        std::io::ErrorKind::InvalidData,
        "Corrupted Data, can't verify hash",
    ));
}

/// Splits a file into a single part.
///
/// # Arguments:
/// 
/// * buf_reader - A mutable reference to a BufReader wrapping the file to be split.
/// * part_num - The part number indicating the current part being split.
/// * file_size - The total size of the file.
/// * reg_chunk_size - The regular chunk size used for splitting the file.
/// * file_hash - The hash of the file.
/// * chunk_count_max - The maximum number of chunks the file can be split into.
/// * user_hash - The user hash.
/// * header - A mutable reference to the Header struct containing header information.
/// * chunk_hash - An optional hash value for the chunk.
///
/// # Returns
///
/// The function returns a Result with the following outcomes:
/// * Ok(byte_vec) - If the file part is successfully split. byte_vec is a vector of bytes containing the header and the data of the part.
/// * Err(error) - If there is an error during the splitting process, including input/output errors.
/// 
pub fn split_file_single(
    buf_reader: &mut BufReader<&mut File>,
    part_num: usize,
    file_size: usize,
    reg_chunk_size: usize,
    file_hash: &str,
    chunk_count_max: u64,
    user_hash: &str,
    header: &mut Header,
    chunk_hash: &Option<Hash>,
) -> Result<Vec<u8>, RError> {
    let mut chunk_size = reg_chunk_size;

    if part_num as u64 == chunk_count_max {
        chunk_size = file_size - (chunk_size * (part_num - 1));
    }

    let start_pos: u64;

    if part_num == 1 {
        start_pos = 0;
    } else {
        start_pos = (part_num as u64 - 1) * reg_chunk_size as u64;
    }
    let mut buffer = vec![0; chunk_size];

    buf_reader
        .seek(SeekFrom::Start(start_pos))
        .map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

    buf_reader
        .read(&mut buffer)
        .map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;

    write_in_header(
        &mut header.fix_header,
        part_num as u64,
        header.chunk_pos_s,
        header.chunk_pos_e,
    );
    write_in_header(
        &mut header.fix_header,
        chunk_count_max,
        header.chunk_max_pos_s,
        header.chunk_max_pos_e,
    );
    write_in_header(
        &mut header.fix_header,
        chunk_size as u64,
        header.chunk_length_pos_s,
        header.chunk_length_pos_e,
    );
    write_hex_in_header(
        &mut header.fix_header,
        user_hash,
        header.user_pos_s,
        header.user_pos_e,
    )?;
    write_hex_in_header(
        &mut header.fix_header,
        file_hash,
        header.file_hash_pos_s,
        header.file_hash_pos_e,
    )?;

    if chunk_hash.is_some() {
        let chunk_hash = get_hash(&buffer, &chunk_hash.as_ref().expect("chunk_hash not set."));
        write_hex_in_header(
            &mut header.fix_header,
            &chunk_hash,
            header.chunk_hash_pos_s.expect("chunk_hash not set."),
            header.chunk_hash_pos_e.expect("chunk_hash not set."),
        )?;
    }

    let mut byte_vec: Vec<u8> = Vec::new();
    byte_vec.extend(header.fix_header.iter());
    byte_vec.extend(buffer.iter());

    return Ok(byte_vec);
}


/// Merges a data vector into a file at the specified position.
///
/// # Arguments
///
/// * output_path - The path of the output file.
/// * byte_vec - The data vector to be merged into the file.
/// * chunk_number - The chunk number indicating the position within the file where the data vector should be merged.
/// * reg_chunk_size - The regular chunk size used for calculating the start position.
///
/// # Returns
///
/// The function returns a Result with two possible outcomes:
/// * Ok(result_path) - If the data vector is successfully merged into the file. result_path is the path of the output file.
/// * Err(error) - If there is an error during the merging process, including file I/O errors.
///
/// # Errors
///
/// The function can return an error if there is an error creating or opening the output file, checking the file size, or performing file I/O operations. The Error type contains details about the error.
/// 
pub fn merge_file_on_path(
    output_path: &str,
    byte_vec: &Vec<u8>,
    chunk_number: u64,
    reg_chunk_size: usize,
) -> Result<String, Error> {
    //create output file in outputDir if not there
    let mut output_file = match OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&output_path)
    {
        Ok(file) => file,
        Err(err) => {
            return Err(err);
        }
    };

    //get start_pos
    let start_pos = reg_chunk_size as u64 * (chunk_number - 1);

    //check file_size
    let output_file_size = output_file.metadata().unwrap().len();


    //fill file with 0
    if output_file_size < start_pos {
        output_file.seek(SeekFrom::End(0)).unwrap();

        let size_to_append = start_pos - output_file_size;

        let zero_byte_vec: [u8; 256000] = [0; 256000];

        for _ in 0..(size_to_append / zero_byte_vec.len() as u64) {
            output_file.write_all(&zero_byte_vec).unwrap();
        }

        let size_modulo = size_to_append % zero_byte_vec.len() as u64;

        if size_modulo != 0 {
            output_file
                .write_all(&zero_byte_vec[0..size_modulo as usize])
                .unwrap();
        }
    }

    output_file.seek(SeekFrom::Start(start_pos)).unwrap();
    output_file.write_all(byte_vec).unwrap();

    return Ok(output_path.to_string());
}





/// Creates a data vector from a file.
///
/// # Arguments
///
/// * path - The path of the file.
/// * chunk_num - The chunk number indicating the position of the data vector within the file.
/// * file_hash - The file hash.
///
/// # Returns
///
/// The function returns a Result with two possible outcomes:
/// * Ok(split_vec) - If the data vector is successfully created from the file. split_vec is the resulting data vector.
/// * Err(error) - If there is an error during the creation of the data vector, including file I/O errors.
///
/// # Errors
///
/// The function can return an error if there is an error opening the file, reading its metadata, or performing file I/O operations. The RError type contains details about the error.
pub fn create_data_vec(path: &str, chunk_num: u64, file_hash: &str) -> Result<Vec<u8>, RError> {
    let mut file = File::open(path)
        .map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?;
    let file_size = metadata(path)
        .map_err(|err| RError::new(RErrorKind::InputOutputError, &err.to_string()))?
        .len();
    let mut buf_reader = BufReader::with_capacity(BUFFER_SIZE, &mut file);
    let max_chunk_count = calc_chunk_count(CHUNK_SIZE, file_size)?;

    let mut header = create_header(
        file_size,
        max_chunk_count,
        &Hash::SIPHASH24,
        &Some(CHUNK_HASH_TYPE),
    );
    let split_vec = split_file_single(
        &mut buf_reader,
        chunk_num as usize,
        file_size as usize,
        CHUNK_SIZE,
        &file_hash,
        max_chunk_count,
        &USER_HASH,
        &mut header,
        &Some(CHUNK_HASH_TYPE),
    )?;

    return Ok(split_vec);
}


