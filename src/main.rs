mod client;
mod protocol;

extern crate core;

use std::{env, io};
use std::fs::File;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpStream};
use chrono::{Duration, Timelike, Utc};
use dryoc::dryocbox::Bytes;
use dryoc::dryocstream::{DryocStream, Header, MutBytes, Pull, Push, Tag};
use dryoc::kx::{KeyPair, Session, SessionKey};
use dryoc::sign::PublicKey;
use rand::{random, Rng, thread_rng};
use socket2::{Domain, SockAddr, Socket, Type};
use crate::client::WaitingClient;

fn main() {
    let mut client = WaitingClient::new().expect("intialization of client failed");

    println!("Your port: {}", client.get_port());

    let mut line = String::new();
    print!("Port to connect to: ");
    io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut line).unwrap();

    let port = u16::from_str_radix(&(line.lines().next().unwrap()), 10).expect("invalid port");

    let mut client = client.connect("0:0:0:0:0:0:0:0", port).expect("unable to connect");

    let mut file = File::open("/home/philipp/Desktop/test.pdf").unwrap();

    let mut buffer: [u8; 1050447] = [0;1050447];

    file.read_exact(buffer.as_mut_slice()).unwrap();

    client.write(&buffer);

    let msg = client.read();
    let mut file = std::fs::OpenOptions::new().append(true).create(true).open(format!("/home/philipp/Desktop/test{}", thread_rng().gen_range(0..1000))).unwrap();
    file.write_all(&msg).expect("TODO: panic message");

    loop  {

    }
}


