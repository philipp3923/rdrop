extern crate core;

use std::{env, io};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpStream};
use chrono::{Duration, Timelike, Utc};
use dryoc::dryocbox::{DryocBox, NewByteArray, Nonce, KeyPair, PublicKey, Mac};
use dryoc::dryocsecretbox::Bytes;
use dryoc::dryocstream::Key;
use socket2::{Domain, SockAddr, Socket, Type};

fn create_socket() -> Result<Socket, String> {
    let socket = Socket::new(Domain::IPV6, Type::STREAM, None).unwrap();
    let mut bind_success = false;

    'port_loop: for port in 2000..3000{
        let bind_ipv6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0)), port);
        match socket.bind(&SockAddr::from(bind_ipv6)) {
            Ok(_) => {bind_success = true; break 'port_loop}
            Err(_) => {continue 'port_loop}
        }
    }

    if !bind_success {
        return Err("No port available".to_string());
    }

    println!("Your Port: {}", socket.local_addr().unwrap().as_socket().unwrap().port());

    return Ok(socket);
}

fn connect(socket: Socket, ipv6: &str, port: u16) -> Result<TcpStream, Socket> {
    let mut parts_str = ipv6.split(":").into_iter();
    let mut parts: [u16; 8] = [0; 8];


    for i in 0..8 as usize {
        let next_str = match parts_str.next() {
            None => return Err(socket),
            Some(num) => num
        };

        parts[i] = match u16::from_str_radix(next_str, 16) {
            Ok(num) => num,
            Err(_) => return Err(socket)
        };
    }

    let ipv6_addr = Ipv6Addr::from(parts);
    let socket_address = SocketAddr::new(IpAddr::from(ipv6_addr), port);

    for _ in 0..1000 {
        let now = Utc::now();
        let target = now.with_nanosecond(0).unwrap() + Duration::seconds(1);
        let diff = target - now;

        println!("Trying connect at {}", target);

        std::thread::sleep(diff.to_std().unwrap());

        if socket.connect(&SockAddr::from(socket_address)).is_ok() {
            let stream = TcpStream::from(socket);

            println!("Connected {}", Utc::now());

            return Ok(stream);
        }
    }

    return Err(socket);
}


fn main() {
    let mut socket = create_socket().unwrap();

    let mut line = String::new();
    print!("Port to connect to: ");
    io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut line).unwrap();

    let port = u16::from_str_radix(&(line.lines().next().unwrap()),10).unwrap();

    let mut stream = connect(socket, "0:0:0:0:0:0:0:0", port).unwrap();

    let my_keypair = KeyPair::gen();
    let my_session_key = Key::gen();
    let my_nonce = Nonce::gen();

    stream.write_all(my_keypair.public_key.as_slice()).unwrap();

    let mut buf: [u8; 32] = [0; 32];

    stream.read_exact(buf.as_mut_slice()).unwrap();

    let partner_public_key = PublicKey::from(buf);

    println!("my_public_key:      {:02X?}", my_keypair.public_key.as_slice());
    println!("partner_public_key: {:02X?}", partner_public_key.as_slice());

    stream.write_all(my_nonce.as_slice()).unwrap();

    let mut buf: [u8; 24] = [0; 24];

    stream.read_exact(buf.as_mut_slice()).unwrap();

    let partner_nonce = Nonce::from(buf);



    let my_session_key_encrypted = DryocBox::encrypt_to_vecbox(&my_session_key, &my_nonce, &partner_public_key, &my_keypair.secret_key).unwrap();

    let mut sodium_box = my_session_key_encrypted.to_vec();

    stream.write_all(sodium_box.as_slice()).unwrap();

    let mut buf = sodium_box.as_mut_slice();

    println!("my_encrypted_session_key:      {:02X?}", buf.as_slice());

    stream.read_exact(buf).unwrap();

    println!("partner_encrypted_session_key: {:02X?}", buf.as_slice());

    let partner_session_key_encrypted: DryocBox<PublicKey, Mac, Vec<u8>> = DryocBox::from_bytes(buf).unwrap();

    let partner_session_key = partner_session_key_encrypted.decrypt_to_vec(&partner_nonce, &partner_public_key, &my_keypair.secret_key).unwrap();

    println!("my_session_key:      {:02X?}", my_session_key.as_slice());
    println!("partner_session_key: {:02X?}", partner_session_key.as_slice());
}
