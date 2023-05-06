use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::net::Ipv6Addr;
use std::time::Duration;

pub mod udp;
pub mod tcp;

pub trait WaitingClient {
    fn get_port(&self) -> u16;
}

pub trait ActiveClient {
    fn split(self) -> (Box<dyn ClientWriter>, Box<dyn ClientReader>);
    fn reader_ref(&mut self) -> Box<&mut dyn ClientReader>;
    fn writer_ref(&mut self) -> Box<&mut dyn ClientWriter>;
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