use std::char::MAX;
use std::io::{Read, Write};
use std::net::TcpStream;
use rsa::{Oaep, PublicKey, RsaPrivateKey, RsaPublicKey};
use rsa::pkcs1::LineEnding;
use rsa::pkcs8::{DecodePublicKey, EncodePublicKey};

const BITS: usize = 4096;
const MAX_MSG_SIZE: usize = BITS/8 - 2 * 256/8 - 2;
const MSG_SIZE: usize = 256;

pub fn generate_key_pair() -> (RsaPrivateKey, RsaPublicKey) {
    let mut rng = rand::thread_rng();

    let private_key = RsaPrivateKey::new(&mut rng, BITS).expect("failed to generate a key");
    let public_key = RsaPublicKey::from(&private_key);

    return (private_key, public_key);
}

pub fn exchange_public_keys(stream: &mut TcpStream, my_public_key: &RsaPublicKey) -> RsaPublicKey{
    let public_key = my_public_key.to_public_key_pem(LineEnding::default()).unwrap();

    println!("MY KEY {}", String::from_utf8_lossy(public_key.as_bytes()));

    stream.write_all(public_key.as_bytes()).unwrap();

    let mut public_key = Vec::from(public_key);

    stream.read_exact(public_key.as_mut_slice()).unwrap();

    let public_key = RsaPublicKey::from_public_key_pem(&std::str::from_utf8(public_key.as_slice()).unwrap()).unwrap();

    return public_key;
}


pub fn test_encryption(stream: &mut TcpStream, my_private_key: &RsaPrivateKey, my_public_key : &RsaPublicKey, public_key: &RsaPublicKey) -> Result<(),()> {
    let mut rng = rand::thread_rng();

    let padding = Oaep::new::<sha2::Sha256>();
    let data = my_public_key.to_public_key_pem(LineEnding::default()).unwrap();
    let mut enc_data = public_key.encrypt(&mut rng, padding, &data.as_bytes()[0..MSG_SIZE]).unwrap();

    stream.write_all(&enc_data.as_slice()).unwrap();

    stream.read_exact(enc_data.as_mut_slice()).unwrap();

    let padding = Oaep::new::<sha2::Sha256>();
    let dec_data = my_private_key.decrypt(padding, enc_data.as_slice()).unwrap();
    let data = public_key.to_public_key_pem(LineEnding::default()).unwrap();


    //println!("{:02X?}",&dec_data.as_slice()[0..MSG_SIZE]);
    //println!("{:02X?}",&data.as_bytes()[0..MSG_SIZE]);

    if dec_data.as_slice()[0..MSG_SIZE] != data.as_bytes()[0..MSG_SIZE] {
        return Err(());
    }

    return Ok(());
}
