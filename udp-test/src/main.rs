mod protocol;
mod udp;
mod udp2;

use crate::protocol::{connect, handshake};



use rsntp::SntpClient;

use std::env;


use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::str::FromStr;

use std::time::{Duration};

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

    let bind_addr = IpAddr::from(Ipv6Addr::from(0)); // 2A02:3038:414:D662:3162:D4EC:B620:89BF
    let partner_addr = IpAddr::from(Ipv6Addr::from_str("0:0:0:0:0:0:0:0").unwrap()); // 0:0:0:0:0:0:0:0
    let local_addr = SocketAddr::new(bind_addr, src_port);
    let mut udp_socket = UdpSocket::bind(&local_addr).unwrap();

    let connect_addr = SocketAddr::new(partner_addr, dst_port);
    udp_socket.connect(connect_addr).unwrap();

    connect(&mut udp_socket).unwrap();
    let _tcp_stream = handshake(udp_socket).unwrap();

    /*let mut c = UdpConnection::new(Some(src_port)).unwrap();

    c.connect(Ipv6Addr::from_str("0:0:0:0:0:0:0:0").unwrap(), dst_port).unwrap();

    let answer = c.send_and_receive(b"Hallo").unwrap();
    println!("{}", String::from_utf8(answer).unwrap());*/
}
