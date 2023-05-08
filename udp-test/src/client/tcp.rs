use std::error::Error;
use std::io::Write;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::thread::sleep;
use std::time::Duration;
use socket2::{Domain, SockAddr, Socket, Type};
use crate::client::{ActiveClient, ClientReader, ClientWriter, WaitingClient};
use crate::protocol::ChangeStateError;

pub struct TcpWaitingClient {
    tcp_socket: Socket
}

impl TcpWaitingClient {
    fn new(port: Option<u16>) -> Result<TcpWaitingClient, Box<dyn Error>>{
        let tcp_socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;

        let sock_addr =  SockAddr::from(SocketAddr::new(IpAddr::from(Ipv6Addr::from(0)), port.unwrap_or(0)));

        tcp_socket.bind(&sock_addr)?;

        Ok(TcpWaitingClient {tcp_socket})
    }

    fn connect(self,
               peer: Ipv6Addr,
               port: u16, wait: Option<Duration>) -> Result<TcpActiveClient, ChangeStateError<Self>> {
        if wait.is_some() {
            sleep(wait.unwrap());
        }

        let sock_addr = SockAddr::from(SocketAddr::new(IpAddr::from(peer), port));

        return match self.tcp_socket.connect(&sock_addr) {
            Ok(_) => Ok(TcpActiveClient::new(self.tcp_socket)),
            Err(err) => Err(ChangeStateError::new(self, Box::new(err))),
        };
    }
}

impl WaitingClient for TcpWaitingClient {
    fn get_port(&self) -> u16 {
        self.tcp_socket.local_addr().unwrap().as_socket().unwrap().port()
    }
}

pub struct TcpActiveClient {
    writer_client: TcpClientWriter,
    reader_client: TcpClientReader
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

    fn new(tcp_socket: Socket) -> TcpActiveClient {
        let tcp_socket_clone = tcp_socket.try_clone().unwrap();

        let reader_client = TcpClientReader::new(tcp_socket);
        let writer_client = TcpClientWriter::new(tcp_socket_clone);

        return TcpActiveClient {reader_client, writer_client};
    }

}

pub struct TcpClientReader {
    tcp_socket: Socket
}

impl TcpClientReader {
    fn new(tcp_socket: Socket) -> TcpClientReader {

        TcpClientReader { tcp_socket }
    }
}

impl ClientReader for TcpClientReader {
    fn try_read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        todo!()
    }

    fn read(&mut self, timeout: Option<std::time::Duration>) -> Result<Vec<u8>, Box<dyn Error>> {
        //peek header size and read exact
        todo!()
    }
}

pub struct TcpClientWriter {
    tcp_socket: Socket
}

impl TcpClientWriter {
    fn new(tcp_socket: Socket) -> TcpClientWriter {
        TcpClientWriter { tcp_socket }
    }

    fn prepare_msg(&mut self, msg: &[u8]) -> Vec<u8> {
        let len = msg.len();
        let mut result = Vec::with_capacity(len + 4 + 4);

        result.extend_from_slice(&(len as u32).to_be_bytes());
        result.extend_from_slice(msg);

        result
    }
}

impl ClientWriter for TcpClientWriter {
    fn write(&mut self, msg: &[u8]) -> Result<(), Box<dyn Error>> {
        let msg = self.prepare_msg(msg);
        match self.tcp_socket.write(&msg) {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Add;
    use std::thread;
    use std::time::SystemTime;
    use crate::client::TimeoutError;
    use super::*;

    fn connect() -> Result<(TcpActiveClient, TcpActiveClient), Box<dyn Error>> {
        let ipv6 = Ipv6Addr::from(1);

        let c1 = TcpWaitingClient::new(None).unwrap();
        let c2 = TcpWaitingClient::new(None).unwrap();

        let p1 = c1.get_port();
        let p2 = c2.get_port();

        let connect_time= SystemTime::now().add(Duration::from_millis(50));

        let thread_c2 = thread::spawn( move || {
            return c2.connect(ipv6, p1, Some(connect_time.duration_since(SystemTime::now()).unwrap())).unwrap();
        });

        let c1 = c1.connect(ipv6, p2, Some(connect_time.duration_since(SystemTime::now()).unwrap()))?;
        let c2 = match thread_c2.join() {
            Ok(c) => c,
            Err(_e) => {return Err(Box::new(TimeoutError(connect_time.duration_since(SystemTime::now())?)));}
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
}