extern crate core;

use std::env;
use std::env::args;
use std::error::Error;

mod connection;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut connection = connection::Connection::new();

    println!("Your port: {}",connection.get_port());

    let mut line = String::new();
    println!("Port to connect to:");
    let b1 = std::io::stdin().read_line(&mut line).unwrap();

    connection.connect("0:0:0:0:0:0:0:1",u16::from_str_radix(&(line.lines().next().unwrap()), 10).unwrap(),100).await.unwrap();

    return Ok(());
}
