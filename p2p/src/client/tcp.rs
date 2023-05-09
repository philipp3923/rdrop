use crate::client::{ActiveClient, ClientReader, ClientWriter, WaitingClient};
use crate::error::{Error as P2pError, ChangeStateError};
use socket2::{Domain, SockAddr, Socket, Type};


use std::io::{Read, Write};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpStream};
use std::thread::sleep;
use std::time::Duration;

pub struct TcpWaitingClient {
    tcp_socket: Socket,
}

impl TcpWaitingClient {
    pub fn new(port: Option<u16>) -> Result<TcpWaitingClient, P2pError> {
        let tcp_socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;

        let sock_addr = SockAddr::from(SocketAddr::new(
            IpAddr::from(Ipv6Addr::from(0)),
            port.unwrap_or(0),
        ));

        tcp_socket.bind(&sock_addr)?;

        Ok(TcpWaitingClient { tcp_socket })
    }

    pub fn connect(
        self,
        peer: Ipv6Addr,
        port: u16,
        wait: Option<Duration>,
    ) -> Result<TcpActiveClient, ChangeStateError<Self>> {
        if wait.is_some() {
            sleep(wait.unwrap());
        }

        let sock_addr = SockAddr::from(SocketAddr::new(IpAddr::from(peer), port));

        return match self.tcp_socket.connect(&sock_addr) {
            Ok(_) => {
                let tcp_stream = TcpStream::from(self.tcp_socket);
                Ok(TcpActiveClient::new(tcp_stream))
            }
            Err(err) => Err(ChangeStateError::new(self, Box::new(err))),
        };
    }
}

impl WaitingClient for TcpWaitingClient {
    fn get_port(&self) -> u16 {
        self.tcp_socket
            .local_addr()
            .unwrap()
            .as_socket()
            .unwrap()
            .port()
    }
}

pub struct TcpActiveClient {
    writer_client: TcpClientWriter,
    reader_client: TcpClientReader,
}

impl ActiveClient for TcpActiveClient {
    type Reader = TcpClientReader;
    type Writer = TcpClientWriter;

    fn split(self) -> (TcpClientWriter, TcpClientReader) {
        (self.writer_client, self.reader_client)
    }

    fn reader_ref(&mut self) -> &mut TcpClientReader {
        &mut self.reader_client
    }

    fn writer_ref(&mut self) -> &mut TcpClientWriter {
        &mut self.writer_client
    }
}

impl TcpActiveClient {
    fn new(tcp_stream: TcpStream) -> TcpActiveClient {
        // non recoverable error. Program should panic
        let tcp_stream_clone = tcp_stream.try_clone().unwrap();

        let reader_client = TcpClientReader::new(tcp_stream);
        let writer_client = TcpClientWriter::new(tcp_stream_clone);

        return TcpActiveClient {
            reader_client,
            writer_client,
        };
    }
}

pub struct TcpClientReader {
    tcp_stream: TcpStream,
}

impl TcpClientReader {
    fn new(tcp_stream: TcpStream) -> TcpClientReader {
        TcpClientReader { tcp_stream }
    }
}

impl ClientReader for TcpClientReader {
    fn try_read(&mut self) -> Result<Vec<u8>, P2pError> {
        self.tcp_stream.set_nonblocking(true)?;
        let msg = self.read(None);
        self.tcp_stream.set_nonblocking(false)?;
        return msg;
    }

    fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, P2pError> {
        self.tcp_stream.set_read_timeout(timeout)?;

        let mut header = [0u8; 4];

        self.tcp_stream.read_exact(header.as_mut_slice())?;

        let size = u32::from_be_bytes(header) as usize;

        let mut msg = Vec::<u8>::with_capacity(size);

        (0..size).for_each(|_i| msg.push(0));

        self.tcp_stream.read_exact(msg.as_mut_slice())?;

        return Ok(msg);
    }
}

pub struct TcpClientWriter {
    tcp_stream: TcpStream,
}

impl TcpClientWriter {
    fn new(tcp_stream: TcpStream) -> TcpClientWriter {
        TcpClientWriter { tcp_stream }
    }

    fn prepare_msg(&mut self, msg: &[u8]) -> Vec<u8> {
        let len = msg.len();
        let mut result = Vec::with_capacity(len + 4);

        result.extend_from_slice(&(len as u32).to_be_bytes());
        result.extend_from_slice(msg);

        result
    }
}

impl ClientWriter for TcpClientWriter {
    fn write(&mut self, msg: &[u8]) -> Result<(), P2pError> {
        let msg = self.prepare_msg(msg);
        match self.tcp_stream.write(&msg) {
            Ok(_) => Ok(()),
            Err(e) => Err(P2pError::from(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Add;
    use std::thread;
    use std::time::SystemTime;
    use crate::error::ErrorKind;

    fn connect() -> Result<(TcpActiveClient, TcpActiveClient), P2pError> {
        let ipv6 = Ipv6Addr::from(1);

        let c1 = TcpWaitingClient::new(None).unwrap();
        let c2 = TcpWaitingClient::new(None).unwrap();

        let p1 = c1.get_port();
        let p2 = c2.get_port();

        let connect_time = SystemTime::now().add(Duration::from_millis(50));

        let thread_c2 = thread::spawn(move || {
            return c2
                .connect(
                    ipv6,
                    p1,
                    Some(connect_time.duration_since(SystemTime::now()).unwrap()),
                )
                .unwrap();
        });

        let c1 = c1.connect(
            ipv6,
            p2,
            Some(connect_time.duration_since(SystemTime::now()).unwrap()),
        )?;
        let c2 = match thread_c2.join() {
            Ok(c) => c,
            Err(_e) => {
                return Err(P2pError::new(ErrorKind::TimedOut));
            }
        };

        Ok((c1, c2))
    }

    #[test]
    fn test_connect() {
        for _ in 0..10 {
            match connect() {
                Ok(_) => return,
                Err(_) => continue,
            }
        }

        connect().unwrap();
    }

    #[test]
    fn test_read_write_string() {
        let mut clients = connect();
        for _ in 0..10 {
            if clients.is_ok() {
                break;
            }
            clients = connect();
        }

        assert!(clients.is_ok());

        let (mut c1, mut c2) = clients.unwrap();

        let c1_msg = b"Das ist ein Test. Diese Nachricht wird von c1 an c2 versendet.";
        let c2_msg = b"Das ist ein Test. Diese Nachricht wird von c2 an c1 versendet.";

        c1.writer_client.write(c1_msg).unwrap();
        c2.writer_client.write(c2_msg).unwrap();

        let c1_recv = c1.reader_client.try_read().unwrap();
        let c2_recv = c2.reader_client.try_read().unwrap();

        assert_eq!(c1_msg.as_slice(), c2_recv.as_slice());
        assert_eq!(c2_msg.as_slice(), c1_recv.as_slice());
    }

    #[test]
    fn test_read_write_string_complex() {
        let mut clients = connect();
        for _ in 0..10 {
            if clients.is_ok() {
                break;
            }
            clients = connect();
        }

        assert!(clients.is_ok());

        let (mut c1, mut c2) = clients.unwrap();

        let c1_msg = b"Das ist ein Test. Diese Nachricht wird von c1 an c2 versendet.";
        let c2_msg = b"Das ist ein Test. Diese Nachricht wird von c2 an c1 versendet.";

        for _ in 0..10 {
            c1.writer_client.write(c1_msg).unwrap();
            c2.writer_client.write(c2_msg).unwrap();

            let c1_recv = c1.reader_client.try_read().unwrap();
            let c2_recv = c2.reader_client.try_read().unwrap();

            assert_eq!(c1_msg.as_slice(), c2_recv.as_slice());
            assert_eq!(c2_msg.as_slice(), c1_recv.as_slice());

            c1.writer_client.write(c1_msg).unwrap();
            c2.writer_client.write(c2_msg).unwrap();

            let timeout = Some(Duration::from_millis(1));

            let c1_recv = c1.reader_client.read(timeout).unwrap();
            let c2_recv = c2.reader_client.read(timeout).unwrap();

            assert_eq!(c1_msg.as_slice(), c2_recv.as_slice());
            assert_eq!(c2_msg.as_slice(), c1_recv.as_slice());
        }
    }
}