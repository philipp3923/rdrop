use chrono::Duration;
use std::io::{empty, Empty, Error};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::num::ParseIntError;
use std::time;
use async_io::Timer;
use chrono::{Timelike, Utc};
use socket2::{Domain, SockAddr, Socket, Type};

pub struct Connection {
    socket: Socket,
    connected: bool
}

impl Connection {

    pub(crate) fn new() -> Connection{
        let socket = Socket::new(Domain::IPV6, Type::STREAM, None).unwrap();

        let bind_ipv6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0)), 0);
        socket.bind(&SockAddr::from(bind_ipv6)).unwrap();

        return Connection {
            socket,
            connected: false
        };
    }

    pub fn get_port(&self) -> u16{
        return self.socket.local_addr().unwrap().as_socket().unwrap().port();
    }

    pub async fn connect(&mut self, ipv6: &str, port: u16, attempts: u8) -> Result<(),String> {
        println!("{}", port);

        let mut parts_str = ipv6.split(":").into_iter();
        let mut parts: [u16; 8] = [0; 8];


        for i in 0..8 as usize {
            let next_str = match parts_str.next() {
                None => return Err("ipv6 address is to short".to_string()),
                Some(num) => num
            };

            parts[i] = match u16::from_str_radix(next_str, 16) {
                Ok(num) => num,
                Err(_) => return Err("Invalid ipv6 address".to_string())
            };
        }

        let ipv6_addr = Ipv6Addr::from(parts);
        let socket_address = SocketAddr::new(IpAddr::from(ipv6_addr), port);

        for _ in 0..attempts {
            let now = Utc::now();
            let target = now.with_nanosecond(0).unwrap() + Duration::seconds(1);
            let diff = target - now;

            Timer::after(time::Duration::from_nanos(diff.num_nanoseconds().unwrap() as u64)).await;

            if self.socket.connect(&SockAddr::from(socket_address)).is_ok() {
                self.connected = true;
                return Ok(());
            }

            println!("Connecting {}", Utc::now());
        }


        return Err("was not able to connect".to_string());
    }


}