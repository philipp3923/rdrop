mod protocol;
mod udp;
mod udp2;

use crate::protocol::{connect, handshake};



use rsntp::SntpClient;

use std::env;


use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::str::FromStr;

use std::time::{Duration};
use crate::udp2::WaitingConnection;

fn main() {
    env::set_var("RUST_BACKTRACE", "full");
    let client = SntpClient::new();
    let result = client.synchronize("ntp1.m-online.net").unwrap();

    let signum = result.clock_offset().signum() as i8;
    let delta: Duration = result.clock_offset().abs_as_std_duration().unwrap();

    println!("delta:  {}", delta.as_nanos());
    println!("signum: {}", signum);

    let args: Vec<String> = env::args().collect();
    let src_port: u16 = args[1].parse().unwrap();
    let dst_port: u16 = args[2].parse().unwrap();


    let partner_addr = Ipv6Addr::from_str("0:0:0:0:0:0:0:0").unwrap();
    let client = WaitingConnection::new(Some(src_port)).unwrap();
    let mut client = client.connect(partner_addr, dst_port, Some(Duration::from_secs(120))).unwrap();

    client.send(b"Hallo mein Freund!", None).unwrap();

    println!("{}", String::from_utf8(client.read(None).unwrap()).unwrap());

}
