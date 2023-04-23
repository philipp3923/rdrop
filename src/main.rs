extern crate core;

use std::env;
use std::env::args;
use std::error::Error;
use rsa::Oaep;
use tokio::io::split;
use tokio::net::TcpStream;
use crate::connection::{ActiveConnection, WaitingConnection};

mod connection;
mod packages;
mod opcode;
mod encryption;

fn main() -> Result<(), ()> {

    let connection = WaitingConnection::new().unwrap();
    println!("Your port: {}", connection.get_port());

    let mut line = String::new();
    println!("Port to connect to:");
    std::io::stdin().read_line(&mut line).unwrap();

    let mut connection = match connection.connect("0:0:0:0:0:0:0:1",u16::from_str_radix(&(line.lines().next().unwrap()), 10).unwrap()) {
        Ok(active_connection) => active_connection,
        Err(_) => return Err(())
    };

    loop {
        println!("SEND:");
        std::io::stdin().read_line(&mut line).unwrap();
        let mut msg: [u8; 256] = [0; 256];
        let line_bytes = line.as_bytes();
        for i in 0..line_bytes.len() {
            if i >= msg.len() {
                break
            }
            msg[i] = line_bytes[i];
        }

        let encrypted_msg = connection.encrypt(&msg);

        connection.send(encrypted_msg.as_slice());

        match connection.next() {
            Ok(_) => {}
            Err(_) => break
        }

        std::io::stdin().read_line(&mut line).unwrap();
    }

    connection.close();
    return Ok(());
}
