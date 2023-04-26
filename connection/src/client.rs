use crate::ip::Address;
use crate::protocol::{exchange_keys, generate_streams, negotiate_roles, Role};
use crate::time::Synchronizer;
use crate::{CONNECT_ATTEMPTS, PORT_RANGE};
use dryoc::dryocstream::{DryocStream, Pull, Push, Tag};
use socket2::{SockAddr, Socket, Type};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::ops::Range;

#[derive(Debug)]
pub struct WaitingClient<A: Address> {
    socket: Socket,
    address: A,
}

impl<A: Address> WaitingClient<A> {
    pub fn new() -> Result<WaitingClient<A>, String> {
        let mut socket = Socket::new(A::get_domain(), Type::STREAM, None).unwrap();

        match WaitingClient::<A>::bind_port(&mut socket, PORT_RANGE) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        let address = match A::from_public() {
            Ok(a) => a,
            Err(e) => return Err(e),
        };

        return Ok(WaitingClient::<A> { socket, address });
    }

    fn bind_port(socket: &mut Socket, port_range: Range<u16>) -> Result<(), String> {
        let ip = A::from_standard().to_socket_addr();

        'port_loop: for port in port_range.clone() {
            let bind = SocketAddr::new(ip, port);
            match socket.bind(&SockAddr::from(bind)) {
                Ok(_) => {
                    return Ok(());
                }
                Err(_) => {
                    continue 'port_loop;
                }
            }
        }

        return Err(format!(
            "no available port found in range {}-{}",
            port_range.start, port_range.end
        ));
    }

    pub fn get_port(&self) -> u16 {
        return self
            .socket
            .local_addr()
            .unwrap()
            .as_socket()
            .unwrap()
            .port();
    }

    pub fn get_address(&self) -> &A {
        return &self.address;
    }

    pub fn connect(
        self,
        ip: A,
        port: u16,
    ) -> Result<(ClientReader, ClientWriter), WaitingClient<A>> {
        let socket_address = SocketAddr::new(ip.to_socket_addr(), port);
        let mut synchronizer = match Synchronizer::new() {
            Ok(t) => t,
            Err(_) => return Err(self),
        };

        for _ in 0..CONNECT_ATTEMPTS {
            std::thread::sleep(synchronizer.wait_time());

            if self.socket.connect(&SockAddr::from(socket_address)).is_ok() {
                let mut tcp_stream = TcpStream::from(self.socket);
                let my_role: Role = negotiate_roles(&mut tcp_stream);
                let (decrypt_key, encrypt_key) = exchange_keys(&mut tcp_stream, &my_role);
                let (pull_stream, push_stream) =
                    generate_streams(&mut tcp_stream, decrypt_key, encrypt_key);
                let tcp_stream_clone = tcp_stream.try_clone().unwrap();

                return Ok((
                    ClientReader::new(tcp_stream, pull_stream),
                    ClientWriter::new(tcp_stream_clone, push_stream),
                ));
            }
        }

        return Err(self);
    }
}

pub struct ClientReader {
    tcp_stream: TcpStream,
    decrypt_stream: DryocStream<Pull>,
}

pub struct ClientWriter {
    tcp_stream: TcpStream,
    encrypt_stream: DryocStream<Push>,
}

impl ClientReader {
    pub fn new(tcp_stream: TcpStream, decrypt_stream: DryocStream<Pull>) -> ClientReader {
        ClientReader {
            tcp_stream,
            decrypt_stream,
        }
    }

    pub fn read(&mut self) -> Vec<u8> {
        let mut message = Vec::new();

        loop {
            let (data, tag) = self.read_package();
            message.extend_from_slice(data.as_slice());

            if tag == Tag::PUSH {
                break;
            }
        }

        return message;
    }

    fn read_package(&mut self) -> (Vec<u8>, Tag) {
        let mut message = Vec::new();
        let mut header_buffer: [u8; 19] = [0; 19];

        self.tcp_stream.read_exact(&mut header_buffer).unwrap();

        let (mut header, tag) = self
            .decrypt_stream
            .pull_to_vec(&header_buffer, None)
            .unwrap();

        let package_size: u16 =
            (header.pop().unwrap() as u16) + ((header.pop().unwrap() as u16) << 8);

        for _ in 0..package_size / 1024 {
            let mut part_buffer: [u8; 1024 + 17] = [0; 1024 + 17];
            self.tcp_stream.read_exact(&mut part_buffer).unwrap();
            let (part, _) = self.decrypt_stream.pull_to_vec(&part_buffer, None).unwrap();
            message.extend_from_slice(part.as_slice());
        }

        if package_size % 1024 != 0 {
            let size = (package_size % 1024 + 17) as usize;
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
    pub fn new(tcp_stream: TcpStream, encrypt_stream: DryocStream<Push>) -> ClientWriter {
        ClientWriter {
            tcp_stream,
            encrypt_stream,
        }
    }

    pub fn write(&mut self, content: &[u8]) {
        let max2power24 = 65535;
        if content.len() <= max2power24 {
            let package = self.build_package(&content, Tag::PUSH);
            self.tcp_stream.write_all(package.as_slice()).unwrap();
            return;
        }

        let overhead = content.len() % max2power24;

        for i in (0..content.len() - max2power24 - overhead).step_by(max2power24) {
            let package = self.build_package(&content[i..i + max2power24], Tag::MESSAGE);
            self.tcp_stream.write_all(package.as_slice()).unwrap();
        }

        let package = self.build_package(
            &content[content.len() - max2power24 - overhead..content.len() - overhead],
            if overhead == 0 {
                Tag::PUSH
            } else {
                Tag::MESSAGE
            },
        );
        self.tcp_stream.write_all(package.as_slice()).unwrap();

        if overhead > 0 {
            let package =
                self.build_package(&content[content.len() - overhead..content.len()], Tag::PUSH);
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

        let mut package: Vec<u8> =
            Vec::with_capacity(content.len() + 2 + 17 + (content.len() / 1024 + 1) * 17);
        let mut header: [u8; 2] = [0; 2];

        header[0] = size[6];
        header[1] = size[7];

        let encrypted_header = self.encrypt_stream.push_to_vec(&header, None, tag).unwrap();

        package.extend_from_slice(&encrypted_header);

        let overhead = content.len() % 1024;

        for i in (0..content.len() - overhead).step_by(1024) {
            let part = &content[i..i + 1024];
            let encrypted_part = self.encrypt_stream.push_to_vec(&part, None, tag).unwrap();
            package.extend_from_slice(&encrypted_part);
        }

        if overhead > 0 {
            let overhead_part = &content[content.len() - overhead..content.len()];
            let encrypted_part = self
                .encrypt_stream
                .push_to_vec(&overhead_part, None, tag)
                .unwrap();
            package.extend_from_slice(&encrypted_part);
        }

        return package;
    }
}
