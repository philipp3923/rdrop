use crate::client::{ActiveClient, ClientReader, ClientWriter, WaitingClient};
use crate::error::{ChangeStateError, Error as P2pError};
use socket2::{Domain, SockAddr, Socket, Type};

use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpStream};

use std::thread::sleep;
use std::time::Duration;
use crate::error;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

pub struct TcpWaitingClient {
    tcp_socket: Socket,
}

impl TcpWaitingClient {
    pub fn new(port: Option<u16>) -> Result<TcpWaitingClient, P2pError> {

        let tcp_socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;

        tcp_socket.set_write_timeout(Some(CONNECT_TIMEOUT))?;

        let sock_addr = SockAddr::from(SocketAddr::new(
            IpAddr::from(Ipv6Addr::from(0)),
            port.unwrap_or(0),
        ));

        tcp_socket.bind(&sock_addr)?;

        Ok(TcpWaitingClient { tcp_socket })
    }

    /// Connects to a peer.
    pub fn connect(
        mut self,
        peer: Ipv6Addr,
        peer_port: u16,
        wait: Option<Duration>,
        timeout: Option<Duration>,
    ) -> Result<TcpActiveClient, ChangeStateError<Self>> {
        let port = self.get_port();

        // drop the old socket and create a new one to prevent socket is busy error.
        let tmp_socket = match Socket::new(Domain::IPV6, Type::STREAM, None) {
            Ok(socket) => socket,
            Err(err) => return Err(ChangeStateError::new(self, Box::new(err))),
        };

        let old_socket = core::mem::replace(&mut self.tcp_socket, tmp_socket);
        drop(old_socket);

        let tcp_socket = match Socket::new(Domain::IPV6, Type::STREAM, None) {
            Ok(socket) => socket,
            Err(err) => return Err(ChangeStateError::new(self, Box::new(err))),
        };

        match tcp_socket.set_write_timeout(Some(CONNECT_TIMEOUT)) {
            Ok(_) => {}
            Err(err) => return Err(ChangeStateError::new(self, Box::new(err))),
        }

        let sock_addr = SockAddr::from(SocketAddr::new(IpAddr::from(Ipv6Addr::from(0)), port));

        match tcp_socket.bind(&sock_addr) {
            Ok(_) => {}
            Err(err) => return Err(ChangeStateError::new(self, Box::new(err))),
        }

        self.tcp_socket = tcp_socket;

        if let Some(wait_duration) = wait {
            sleep(wait_duration);
        }

        let peer_addr = SockAddr::from(SocketAddr::new(IpAddr::from(peer), peer_port));

        let connect_result = self
            .tcp_socket
            .connect_timeout(&peer_addr, timeout.unwrap_or(Duration::from_secs(1)));

        match connect_result {
            Ok(_) => {
                let tcp_stream = TcpStream::from(self.tcp_socket);
                Ok(TcpActiveClient::new(tcp_stream))
            }
            Err(err) => {
                println!("{}", err);
                Err(ChangeStateError::new(self, Box::new(err)))
            }
        }
    }
}

impl WaitingClient for TcpWaitingClient {
    /// Returns the port the socket is bound to.
    fn get_port(&self) -> u16 {
        self.tcp_socket
            // should panic if it fails. This is fine.
            .local_addr()
            .expect("Failed to retrieve local address")
            .as_socket()
            .expect("Failed to parse socket")
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

        match self.tcp_stream.read_exact(header.as_mut_slice()) {
            Ok(_) => {}
            Err(err) => {

                return if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut {
                    Err(P2pError::new(error::ErrorKind::TimedOut))
                } else {
                    println!("{}", err);
                    println!("{}", err.kind());
                    Err(P2pError::new(error::ErrorKind::CommunicationFailed))
                }
            }
        };

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
        self.tcp_stream.write_all(&msg)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::error::ErrorKind;
    use std::ops::Add;
    use std::thread;
    use std::time::SystemTime;

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
                    Some(Duration::from_millis(50)),
                )
                .unwrap();
        });

        let c1 = c1.connect(
            ipv6,
            p2,
            Some(connect_time.duration_since(SystemTime::now()).unwrap()),
            Some(Duration::from_millis(50)),
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

        let c1_msg = [0x1, 0x2];
        let c2_msg = [0x3, 0x4];

        c1.writer_client.write(c1_msg.as_slice()).unwrap();
        c2.writer_client.write(c2_msg.as_slice()).unwrap();

        let c1_recv = c1.reader_client.try_read().unwrap();
        let c2_recv = c2.reader_client.try_read().unwrap();

        assert_eq!(c1_msg.as_slice(), c2_recv.as_slice());
        assert_eq!(c2_msg.as_slice(), c1_recv.as_slice());
    }

    #[test]
    fn test_timeout() {
        let mut clients = connect();
        for _ in 0..10 {
            if clients.is_ok() {
                break;
            }
            clients = connect();
        }

        let (mut c1, mut c2) = clients.unwrap();

        assert!(c1.reader_client.read(Some(Duration::from_millis(100))).is_err());
        assert!(c2.reader_client.read(Some(Duration::from_millis(100))).is_err());
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
