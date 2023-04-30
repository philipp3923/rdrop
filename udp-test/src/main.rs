mod protocol;

use std::env;
use std::fmt::format;
use std::fs::read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream, UdpSocket};
use std::str::FromStr;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};
use connection::time::Synchronizer;
use rand::{Rng, thread_rng};
use rsntp::SntpClient;
use socket2::Socket;
use crate::protocol::{connect, handshake};

fn main() {
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
    let partner_addr = IpAddr::from(Ipv6Addr::from_str("0:0:0:0:0:0:0:0").unwrap());
    let local_addr = SocketAddr::new(bind_addr, src_port);
    let mut udp_socket = UdpSocket::bind(&local_addr).unwrap();

    let connect_addr = SocketAddr::new(partner_addr, dst_port);
    udp_socket.connect(connect_addr).unwrap();

    connect(&mut udp_socket).unwrap();
    let tcp_stream = handshake(udp_socket).unwrap();
}