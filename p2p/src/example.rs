use crate::client::{ClientReader, ClientWriter};
use crate::protocol::Connection;
use std::net::Ipv6Addr;
use std::str::FromStr;
use std::time::Duration;

#[allow(dead_code)]
fn main() {
    let timeout = Duration::from_secs(60);

    let ipv6 = Ipv6Addr::from_str("ENTER IPV6 ADDRESS HERE").unwrap();

    let connection = Connection::new(Some(2000)).unwrap();

    let connection = connection
        .connect(ipv6, 2000, Some(timeout), Some(timeout))
        .unwrap();

    let connection = connection.encrypt().unwrap();

    let connection = connection.upgrade_direct().unwrap();

    let (mut writer, mut reader) = connection.accept();

    writer.write(b"Das ist ein Test").unwrap();

    let response = reader.read(Some(timeout)).unwrap();

    println!("{}", String::from_utf8(response).unwrap());
}
