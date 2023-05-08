mod protocol_old;
mod client;
mod protocol;

use crate::protocol_old::{connect as other_connect, handshake};



use rsntp::SntpClient;

use std::{env, thread};


use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::str::FromStr;

use std::time::Duration;
use crate::client::{ActiveClient, ClientReader, ClientWriter, EncryptedClient};
use crate::client::udp::UdpWaitingClient;
use crate::protocol::{Active, Connection, Plain, Udp, Waiting};

fn main() {
    env::set_var("RUST_BACKTRACE", "full");

    let timeout = Duration::from_secs(60);

    let ipv6 = Ipv6Addr::from_str("2001:7c7:2159:2f00:d7ca:9f5:bd5c:e681").unwrap();

    let connection = Connection::new(Some(2000)).unwrap();

    let connection = connection.connect(ipv6, 2000, Some(timeout), Some(timeout)).unwrap();

    let connection = connection.encrypt().unwrap();

    // let connection = connection.upgrade().unwrap();

    let (mut writer, mut reader) = connection.accept();

    writer.write(b"WHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHIWHALLAHI").unwrap();

    let response = reader.read(Some(timeout)).unwrap();

    println!("{}", String::from_utf8(response).unwrap());
}
