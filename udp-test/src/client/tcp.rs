use std::error::Error;
use std::net::Ipv6Addr;
use chrono::Duration;
use socket2::Socket;
use crate::client::{ActiveClient, ClientReader, ClientWriter, WaitingClient};

pub struct TcpWaitingClient {
    tcp_socket: Socket
}

impl TcpWaitingClient {
    fn new(port: Option<u16>) -> Result<TcpWaitingClient, Box<dyn Error>>{
        todo!()
    }

    fn connect(self,
               peer: Ipv6Addr,
               port: u16, wait: Option<Duration>) -> Result<TcpActiveClient, Box<dyn Error>> {
        todo!()
    }
}

impl WaitingClient for TcpWaitingClient {
    fn get_port(&self) -> u16 {
        todo!()
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
        todo!()
    }

    fn reader_ref(&mut self) -> &mut TcpClientReader {
        todo!()
    }

    fn writer_ref(&mut self) -> &mut TcpClientWriter {
        todo!()
    }

    fn max_msg_len(&self) -> u32 {
        todo!()
    }
}

pub struct TcpClientReader {
    tcp_socket: Socket
}

impl TcpClientReader {
    fn new(tcp_socket: Socket) -> TcpClientReader {
        todo!()
    }
}

impl ClientReader for TcpClientReader {
    fn try_read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        todo!()
    }

    fn read(&mut self, timeout: Option<std::time::Duration>) -> Result<Vec<u8>, Box<dyn Error>> {
        todo!()
    }
}

pub struct TcpClientWriter {
    tcp_socket: Socket
}

impl TcpClientWriter {
    fn new(tcp_socket: Socket) -> TcpClientWriter {
        todo!()
    }
}

impl ClientWriter for TcpClientWriter {
    fn write(&mut self, msg: &[u8]) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}