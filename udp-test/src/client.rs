use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::net::Ipv6Addr;
use std::time::Duration;
use dryoc::dryocstream::{DryocStream, Pull, Push};

pub mod udp;
pub mod tcp;

pub trait WaitingClient {
    fn get_port(&self) -> u16;
}

pub trait ActiveClient {
    type Reader: ClientReader;
    type Writer: ClientWriter;

    fn split(self) -> (Self::Writer, Self::Reader);
    fn reader_ref(&mut self) -> &mut Self::Reader;
    fn writer_ref(&mut self) -> &mut Self::Writer;
    fn max_msg_len(&self) -> u32;
}

pub trait ClientReader {
    fn try_read(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, Box<dyn Error>>;
}

pub trait ClientWriter {
    fn write(&mut self, msg: &[u8]) -> Result<(), Box<dyn Error>>;
}

#[derive(Clone, Debug)]
pub struct TimeoutError(pub Duration);

impl Display for TimeoutError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "timelimit {:#?} exceeded", self.0)
    }
}

impl Error for TimeoutError {
    fn description(&self) -> &str {
        "The given timelimit was exceeded"
    }
}


pub struct EncryptedClient<AC: ActiveClient> {
    active_client: AC
}

impl<AC: ActiveClient> EncryptedClient<AC> {

    pub fn new(active_client: AC) -> Result<(EncryptedReader<AC::Reader>, EncryptedWriter<AC::Writer>), Box<dyn Error>> {
        let (writer, reader) = active_client.split();


        todo!()
    }
}


pub struct EncryptedReader<CR: ClientReader>{
    pull_stream: DryocStream<Pull>,
    client_reader: CR
}

impl<CR: ClientReader> EncryptedReader<CR> {

    fn new(pull_stream: DryocStream<Pull>, client_reader: CR) -> EncryptedReader<CR> {
        EncryptedReader {client_reader, pull_stream}
    }
}

impl<CR: ClientReader> ClientReader for EncryptedReader<CR> {
    fn try_read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        todo!()
    }

    fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, Box<dyn Error>> {
        todo!()
    }
}

pub struct EncryptedWriter<CW: ClientWriter> {
    push_stream: DryocStream<Push>,
    client_writer: CW
}

impl<CW: ClientWriter> EncryptedWriter<CW> {

    fn new(push_stream: DryocStream<Push>, client_writer: CW) -> EncryptedWriter<CW> {
        EncryptedWriter {client_writer, push_stream}
    }

}

impl<CW: ClientWriter> ClientWriter for EncryptedWriter<CW> {
    fn write(&mut self, msg: &[u8]) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}
