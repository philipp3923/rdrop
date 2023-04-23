use std::future::Future;
use chrono::Duration;
use std::io::{empty, Empty, Error, Read, Write};
use std::net::{IpAddr, Ipv6Addr, Shutdown, SocketAddr, TcpStream};
use std::num::ParseIntError;
use std::time;
use async_io::Timer;
use chrono::{Timelike, Utc};
use crc::{Crc, CRC_32_ISCSI};
use rand::rngs::ThreadRng;
use rand::thread_rng;
use rsa::{Oaep, PublicKey, RsaPrivateKey, RsaPublicKey};
use socket2::{Domain, SockAddr, Socket, Type};
use tokio::io::{AsyncReadExt, AsyncWriteExt, split};
use crate::encryption::{exchange_public_keys, generate_key_pair, test_encryption};
use crate::opcode::Opcode;
use crate::packages::{Package};

pub struct WaitingConnection {
    socket: Socket
}

pub struct ActiveConnection {
    stream: TcpStream,
    my_public_key: RsaPublicKey,
    my_private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
    rng: ThreadRng,
    crc: Crc<u32>
}

const START_PORT: u16 = 2000;
const PORT_RANGE: u16 = 100;
const CONNECTION_ATTEMPTS: u16 = 300;

impl WaitingConnection {

    pub fn new() -> Result<WaitingConnection, String> {
        let socket = Socket::new(Domain::IPV6, Type::STREAM, None).unwrap();
        let mut bind_success = false;

        'port_loop: for port in START_PORT..START_PORT+PORT_RANGE {
            let bind_ipv6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0)), port);
            match socket.bind(&SockAddr::from(bind_ipv6)) {
                Ok(_) => {bind_success = true; break 'port_loop}
                Err(_) => {continue 'port_loop}
            }
        }

        if !bind_success {
            return Err("No port available".to_string());
        }

        return Ok(WaitingConnection {socket});
    }

    pub fn get_port(&self) -> u16 {
        return self.socket.local_addr().unwrap().as_socket().unwrap().port();
    }

    pub fn connect(self, ipv6: &str, port: u16) -> Result<ActiveConnection, WaitingConnection> {
        let mut parts_str = ipv6.split(":").into_iter();
        let mut parts: [u16; 8] = [0; 8];


        for i in 0..8 as usize {
            let next_str = match parts_str.next() {
                None => return Err(self),
                Some(num) => num
            };

            parts[i] = match u16::from_str_radix(next_str, 16) {
                Ok(num) => num,
                Err(_) => return Err(self)
            };
        }

        let ipv6_addr = Ipv6Addr::from(parts);
        let socket_address = SocketAddr::new(IpAddr::from(ipv6_addr), port);

        for _ in 0..CONNECTION_ATTEMPTS {
            let now = Utc::now();
            let target = now.with_nanosecond(0).unwrap() + Duration::seconds(1);
            let diff = target - now;

            std::thread::sleep(diff.to_std().unwrap());

            if self.socket.connect(&SockAddr::from(socket_address)).is_ok() {
                let mut stream = TcpStream::from(self.socket);
                let key_pair = generate_key_pair();
                let public_key = exchange_public_keys(&mut stream, &key_pair.1);
                test_encryption(&mut stream, &key_pair.0, &key_pair.1, &public_key).expect("Encryption not to fail.");
                return Ok(ActiveConnection { stream, my_public_key: key_pair.1, my_private_key: key_pair.0, public_key, rng: thread_rng(), crc: Crc::<u32>::new(&CRC_32_ISCSI)});
            }
        }

        return Err(self);
    }

}


impl ActiveConnection {

    pub fn send(&mut self, msg: &[u8]) {
        println!("SEND:\n{:02X?}",msg);
        self.stream.write_all(&msg).unwrap();
    }

    pub fn next(&mut self) -> Result<String, String> {
        let mut buf: [u8; 512] = [0; 512];

        match self.stream.read_exact(&mut buf) {
            Ok(_) => {},
            Err(e) => return Err(format!("Cannot read. {}", e))
        }
        println!("RECEIVED:\n{:02X?}",buf);
        let mut decrypted_msg = self.decrypt(&buf).unwrap();

        println!("RECEIVED:\n{}", String::from_utf8_lossy(decrypted_msg.as_slice()));

        /*let message: Package = match Opcode::try_from(opcode_buf[0]) {
            Ok(opcode) => Package::try_from((opcode, &mut self.stream)).unwrap(),
            Err(_) => return Err(format!("Received illegal opcode: {:02X?}", opcode_buf[0])),
        };*/


        return Ok(format!("hallo"));
    }

    pub fn encrypt(&mut self, data: &[u8; 256]) -> Vec<u8> {
        let checksum = self.crc.checksum(data);
        let mut data = Vec::from(data.as_slice());
        let padding = Oaep::new::<sha2::Sha256>();

        data.extend_from_slice(checksum.to_be_bytes().as_slice());

        return self.public_key.encrypt(&mut self.rng, padding, data.as_slice()).unwrap();
    }

    pub fn decrypt(&mut self, data: &[u8; 512]) -> Result<Vec<u8>, Vec<u8>> {
        let padding = Oaep::new::<sha2::Sha256>();
        let mut dec_data = match self.my_private_key.decrypt(padding, data) {
            Ok(msg) => msg,
            Err(error) => {println!("{}",error); return Ok(Vec::new());}
        };

        let checksum = self.crc.checksum(&data.as_slice()[0..256]);

        if checksum.to_be_bytes() != data.as_slice()[256..260] {
            return Ok(dec_data.split_off(256));
        }

        return Ok(dec_data.split_off(256));
    }

    pub fn close(mut self) {
        Socket::from(self.stream).shutdown(Shutdown::Both).unwrap();
    }

}