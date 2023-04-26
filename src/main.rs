mod client;
mod package;

extern crate core;

use std::io;

use std::io::{Read, Write};

use dryoc::dryocbox::Bytes;

use connection::client::WaitingClient;
use connection::ip::{Address, Ipv4};

fn main() {
    example::<Ipv4>();
}

fn example<A: Address>() {
    let client = WaitingClient::<A>::new().expect("creating client failed");

    println!("Your ip: {}", client.get_address());
    println!("Your port: {}", client.get_port());

    // Read Ip from Input
    let mut line = String::new();
    print!("Connect to ip: ");
    io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut line).unwrap();
    let ip = Address::from_str(&line).expect("failed to parse ip address");

    // Read Port from Input
    let mut line = String::new();
    print!("Connect to port: ");
    io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut line).unwrap();
    let port =
        u16::from_str_radix(&(line.lines().next().unwrap()), 10).expect("failed to parse port");

    let (mut reader, mut writer) = client.connect(ip, port).expect("connection failed");

    writer.write("Hallo!".as_bytes());

    let response = reader.read();
    let response = String::from_utf8_lossy(response.as_slice());

    println!("{}", response);
}
