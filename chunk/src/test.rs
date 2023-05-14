use std::fs::{File, metadata};
use std::io::BufReader;

mod hash;
mod general;
mod error;
mod offer;
mod order;
mod file;

use offer::offer::{create_offer, read_offer, OFFER_REGEX};

use crate::general::general::{AppSettings, write_to_log_file, read_log_file, create_header, separate_header, append_header, HeaderByte, calc_chunk_count, read_send_header, check_chunk_hash, LOGGER_REGEX, validate_log_file, get_filename_from_dir, validate_file, CHUNK_SIZE, CHUNK_HASH_TYPE, BUFFER_SIZE, USER_HASH, create_stop, read_stop};
use crate::hash::hash::{get_file_hash, Hash};
use crate::offer::offer::{create_offer_vec, read_offer_vec};
use crate::order::order::{create_order, read_order, ORDER_REGEX, create_order_from_logfile, create_order_from_offer};
use crate::file::file::{split_file_single, merge_file, r_w_data_vec, create_data_vec};


fn main() {
    
    let vec = create_stop("aabbccddeeff00112233445566778899").unwrap();

    let a = read_stop(&vec).unwrap();

    println!("a: {}", a);

    //vorgang: 0 App öffnet sich und erstellt ein appSettings-Struct
    let mut app_settings = AppSettings::default();
    app_settings.file_hash = Hash::SIPHASH24;

    let u1_file_hash:Hash = Hash::SIPHASH24;
    //vorgang: 1.A User_1 erstellt offer und gibt es zurück
    /*
        Benötigt eingabe von außen -> PATH

        Beinhaltet folgende Datan
        header 00000001,
        file-name,
        file_size,
        file_hash_algorithm,
        file_hash
    */
    let u1_path = "C:\\Users\\sstiegler\\projects\\chunker\\src\\file\\file\\The Rust Programming Language.pdf";    
    //let path = "C:\\Users\\sstiegler\\projects\\chunker\\src\\file\\file\\Testfile.pdf";    
    //let path = "C:\\Users\\sstiegler\\Downloads\\achalmhof_fotobox-bilder-15-04-23_2023-05-08_1455.zip";
    //let u1_path = "C:\\Users\\sstiegler\\Downloads\\zipfotobox.zip";
    let u1_file_hash:Hash = Hash::SIPHASH24;


    let offer_byte_vec = create_offer_vec(&u1_file_hash, &u1_path).unwrap();
    println!("offer_vec: {:?}", &offer_byte_vec);


    //Vorgang 2.A User 2 liest offer und gibt ein Offer-Objekt zurück
    /*
        benötigt offer als String / Byte-stream
    */
    let u2_offer = read_offer_vec(&offer_byte_vec).unwrap();
    println!("offer: {:?}",u2_offer);


    //Vorgang 3.A User 2 erstellt Order bytestream
    /*
        creates order-byte-vec from offer
        Beinhaltet
        header 00000010
        chunk_size,
        file_hash_algorithm,
        file_hash,
        file_name,
        start_pos,
        end_pos,

        chunk-hash-alg deprecated, get recreated via chunk-hash-length
    */
    let u2_output_dir = "./output";

    let mut order_byte_vec = create_order_from_offer(CHUNK_SIZE, &u2_offer.hash_type, &Some(CHUNK_HASH_TYPE), u2_output_dir, &u2_offer).unwrap();
    println!("order_byte_vec: {:?}", &order_byte_vec);


    //Vorgang 4.A User 1 liest order-byte-vec und erstellt ein order-objekt
    /*
        benötigt REGEX für Order
        benötigt byte-vec

        wenn order.start_num == order.end_num == 0
        -> nichts machen!
    */
    let u1_order = read_order(&mut order_byte_vec).unwrap();
    println!("order: {:?}", u1_order);

    // Vorgang 5.A User 1 loopt über alle
    /*
        benötigt folgende aktionen
        - erstelle bufreader -> dafür öffne file
        - get file-size
        - get max chunk-count
        - create header!
    */
    let u1_test_chunk_num = 1;

    //let split_byte_vec = create_data_vec(&u1_path, &u1_order, u1_test_chunk_num).unwrap();





    //simulate all splits and save them in an array...    
    for part_num in u1_order.start_num..=u1_order.end_num{

        //let split_byte_vec = create_data_vec(&u1_path, &u1_order, part_num).unwrap();
        //let _header_data = r_w_data_vec(&split_byte_vec, &u2_output_dir).unwrap();
    }

    //Vorgang 6.A User 2 Liest byte-vec, schreibt daten in file und ergänzt log-file
    /*
        dafür:
        - split header and data byte-vec
        - read header_data

    */

    //let header_data = r_w_data_vec(&split_byte_vec, &u2_output_dir).unwrap();
    //println!("header_data of written {:?}", header_data);


    //Vorgang 6.D User 2 kontrolliert logfiles, ob alle daten eingeschrieben wurden
    /*
        erhält größte und kleinste position, die neu gesendet werden müssen...
        bei 0,0 alles gut und keine files fehlen
        kann somit wieder order aufrufen und neu bestellen
    */

    //let (startpos, endpos) = validate_file(&u2_output_dir, &header_data.file_hash).unwrap();

/* 

    println!("startpos for order: {}", startpos);
    println!("endeepos for order: {}", endpos);

    // Vorgang 6.E User 2 kann via logfile die teile herauslesen, die fehlen und schickt eine neue order an User 1
    /*
        braucht als param den path
    */


    let outpath = format!("{}/{}",&app_settings.output_dir, &header_data.file_hash);
    let file_name = get_filename_from_dir(&outpath).unwrap();
    let logfile_path = format!("{}/{}.rdroplog", outpath, &file_name);



    let order_from_offer = create_order_from_logfile(&logfile_path, BUFFER_SIZE, CHUNK_SIZE).unwrap();
    println!("order_from_offer: {:?}", order_from_offer);
    */

}