use chrono::Utc;
use dryoc::dryocbox::Bytes;
use dryoc::dryocstream::{DryocStream, Header, MutBytes, Pull, Push};
use dryoc::kx::{KeyPair, Session, SessionKey};
use dryoc::sign::PublicKey;
use rand::{thread_rng, Rng};
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
pub(crate) enum Role {
    Server,
    Client,
}

pub(crate) fn negotiate_roles(stream: &mut TcpStream) -> Role {
    let mut rng = thread_rng();
    std::thread::sleep(core::time::Duration::new(0, rng.gen_range(0..1000)));

    let my_timestamp = Utc::now().timestamp_nanos();

    let mut buffer = my_timestamp.to_be_bytes();

    stream.write_all(&buffer).unwrap();

    stream.read_exact(&mut buffer).unwrap();

    let partner_timestamp = i64::from_be_bytes(buffer);

    //println!("my_timestamp:      {}",my_timestamp);
    //println!("partner_timestamp: {}",partner_timestamp);

    if partner_timestamp == my_timestamp {
        return negotiate_roles(stream);
    }

    if partner_timestamp > my_timestamp {
        return Role::Server;
    }

    return Role::Client;
}

pub(crate) fn exchange_keys(stream: &mut TcpStream, role: &Role) -> (SessionKey, SessionKey) {
    let my_keypair = KeyPair::gen();

    stream
        .write_all(my_keypair.public_key.as_slice())
        .expect("writing public_key failed");

    let mut buf: [u8; 32] = [0; 32];

    stream
        .read_exact(&mut buf)
        .expect("reading public_key failed");

    let partner_public_key = PublicKey::from(&buf);

    let my_session_keys = match role {
        Role::Server => Session::new_server_with_defaults(&my_keypair, &partner_public_key)
            .expect("compute session failed"),
        Role::Client => Session::new_client_with_defaults(&my_keypair, &partner_public_key)
            .expect("compute session failed"),
    };

    return my_session_keys.into_parts();
}

pub(crate) fn generate_streams(
    stream: &mut TcpStream,
    decryption_key: SessionKey,
    encryption_key: SessionKey,
) -> (DryocStream<Pull>, DryocStream<Push>) {
    let (push_stream, mut header): (_, Header) = DryocStream::init_push(&encryption_key);

    stream.write_all(header.as_slice()).unwrap();

    stream.read_exact(header.as_mut_slice()).unwrap();

    let pull_stream = DryocStream::init_pull(&decryption_key, &header);

    return (pull_stream, push_stream);
}
