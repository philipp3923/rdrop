use std::io::{Read, Write};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpStream};
use chrono::{Duration, Timelike, Utc};
use dryoc::dryocbox::Bytes;
use dryoc::dryocstream::{DryocStream, Header, MutBytes, Pull, Push, Tag};
use dryoc::kx::{KeyPair, PublicKey, Session, SessionKey};
use rand::{Rng, thread_rng};
use socket2::{Domain, SockAddr, Socket, Type};
use crate::protocol::{exchange_keys, generate_streams, negotiate_roles, Role};

#[derive(Debug)]
pub struct WaitingClient {
    socket: Socket
}

impl WaitingClient {

    pub fn new() -> Result<WaitingClient, String> {
        let socket = Socket::new(Domain::IPV6, Type::STREAM, None).unwrap();
        let mut bind_success = false;

        'port_loop: for port in 2000..3000 {
            let bind_ipv6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0)), port);
            match socket.bind(&SockAddr::from(bind_ipv6)) {
                Ok(_) => {
                    bind_success = true;
                    break 'port_loop;
                }
                Err(_) => { continue 'port_loop; }
            }
        }

        if !bind_success {
            return Err("No port available".to_string());
        }

        return Ok(WaitingClient {socket});
    }

    pub fn get_port(&self) -> u16 {
        return self.socket.local_addr().unwrap().as_socket().unwrap().port();
    }

    pub fn connect(self, ipv6: &str, port: u16) -> Result<ActiveClient, WaitingClient> {
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

        for _ in 0..1000 {
            let now = Utc::now();
            let target = now.with_nanosecond(0).unwrap() + Duration::seconds(1);
            let diff = target - now;

            println!("Trying connect at {}", target);

            std::thread::sleep(diff.to_std().unwrap());

            if self.socket.connect(&SockAddr::from(socket_address)).is_ok() {
                let stream = TcpStream::from(self.socket);

                println!("Connected {}", Utc::now());

                return Ok(ActiveClient::new(stream));
            }
        }

        return Err(self);
    }

}

pub struct ActiveClient {
    tcp_stream: TcpStream,
    decrypt_stream: DryocStream<Pull>,
    encrypt_stream: DryocStream<Push>
}

impl ActiveClient{

    pub fn new(mut tcp_stream: TcpStream) -> ActiveClient {
        let my_role : Role = negotiate_roles(&mut tcp_stream);
        let (decrypt_key, encrypt_key) = exchange_keys(&mut tcp_stream, &my_role);
        let (pull_stream, push_stream) = generate_streams(&mut tcp_stream, decrypt_key, encrypt_key);
        return ActiveClient {tcp_stream, decrypt_stream: pull_stream, encrypt_stream: push_stream};
    }

    pub fn split(self) -> (ClientReader, ClientWriter) {
        let tcp_stream_clone = self.tcp_stream.try_clone().unwrap();
        return (ClientReader {tcp_stream: self.tcp_stream, decrypt_stream: self.decrypt_stream}, ClientWriter {tcp_stream: tcp_stream_clone,encrypt_stream: self.encrypt_stream});
    }
}

pub struct ClientReader {
    tcp_stream: TcpStream,
    decrypt_stream: DryocStream<Pull>,
}

pub struct ClientWriter {
    tcp_stream: TcpStream,
    encrypt_stream: DryocStream<Push>
}


impl ClientReader {
    pub fn read(&mut self) -> Vec<u8> {
        let mut message = Vec::new();

        loop {
            let (data, tag) = self.read_package();
            message.extend_from_slice(data.as_slice());

            if tag == Tag::PUSH {
                break
            }
        }

        return message;
    }

    fn read_package(&mut self) -> (Vec<u8>, Tag) {
        let mut message = Vec::new();
        let mut header_buffer: [u8; 19] = [0; 19];

        self.tcp_stream.read_exact(&mut header_buffer).unwrap();

        let (mut header, mut tag) = self.decrypt_stream.pull_to_vec(&header_buffer, None).unwrap();

        let package_size: u16 = (header.pop().unwrap() as u16) + ((header.pop().unwrap() as u16) << 8);

        for _ in 0..package_size/1024 {
            let mut part_buffer: [u8; 1024 + 17] = [0; 1024 + 17];
            self.tcp_stream.read_exact(&mut part_buffer).unwrap();
            let (part, _) = self.decrypt_stream.pull_to_vec(&part_buffer, None).unwrap();
            message.extend_from_slice(part.as_slice());
        }

        if package_size % 1024 != 0 {
            let size = (package_size % 1024 +17) as usize;
            let mut part_buffer: Vec<u8> = Vec::with_capacity(size);
            for _ in 0..size {
                part_buffer.push(0);
            }
            self.tcp_stream.read_exact(&mut part_buffer).unwrap();
            let (part, _) = self.decrypt_stream.pull_to_vec(&part_buffer, None).unwrap();
            message.extend_from_slice(part.as_slice());
        }

        return (message, tag);
    }
}

impl ClientWriter {
    pub fn write(&mut self, content: &[u8]) {
        let max2power24 = 65535;
        if content.len() <= max2power24 {
            let package = self.build_package(&content, Tag::PUSH);
            self.tcp_stream.write_all(package.as_slice()).unwrap();
            return;
        }

        let overhead = content.len() % max2power24;

        for i in (0..content.len() - max2power24 - overhead).step_by(max2power24){
            let package = self.build_package(&content[i..i+max2power24], Tag::MESSAGE);
            self.tcp_stream.write_all(package.as_slice()).unwrap();
        }

        let package = self.build_package(&content[content.len()-max2power24-overhead..content.len()-overhead], if overhead == 0 {Tag::PUSH } else {Tag::MESSAGE});
        self.tcp_stream.write_all(package.as_slice()).unwrap();

        if overhead > 0 {
            let package = self.build_package(&content[content.len()-overhead..content.len()], Tag::PUSH);
            self.tcp_stream.write_all(package.as_slice()).unwrap();
        }
    }

    fn build_package(&mut self, content: &[u8], tag: Tag) -> Vec<u8> {
        let size = content.len().to_be_bytes();
        assert_eq!(size[0], 0);
        assert_eq!(size[1], 0);
        assert_eq!(size[2], 0);
        assert_eq!(size[3], 0);
        assert_eq!(size[4], 0);
        assert_eq!(size[5], 0);

        let mut package: Vec<u8> = Vec::with_capacity(content.len() + 2 + 17 + (content.len()/1024 + 1) * 17 );
        let mut header: [u8; 2] = [0; 2];

        header[0] = size[6];
        header[1] = size[7];

        let encrypted_header = self.encrypt_stream.push_to_vec(&header, None, tag).unwrap();

        package.extend_from_slice(&encrypted_header);

        let overhead = content.len() % 1024;

        for i in(0..content.len() - overhead).step_by(1024) {
            let part = &content[i..i+1024];
            let encrypted_part = self.encrypt_stream.push_to_vec(&part, None, tag).unwrap();
            package.extend_from_slice(&encrypted_part);
        }

        if overhead > 0 {
            let overhead_part = &content[content.len()-overhead..content.len()];
            let encrypted_part = self.encrypt_stream.push_to_vec(&overhead_part, None, tag).unwrap();
            package.extend_from_slice(&encrypted_part);
        }

        return package;
    }
}