use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::net::Ipv6Addr;
use std::time::Duration;
use dryoc::dryocstream::{DryocStream, Pull, Push, Tag};
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

    pub fn new(active_client: AC, push_stream: DryocStream<Push>, pull_stream: DryocStream<Pull>) -> (EncryptedReader<AC::Reader>, EncryptedWriter<AC::Writer>) {
        let (writer, reader) = active_client.split();

        let encrypted_reader = EncryptedReader::new(pull_stream, reader);
        let encrypted_writer = EncryptedWriter::new(push_stream, writer);

        (encrypted_reader, encrypted_writer)
    }
}

const BLOCK_SIZE: usize = 1024;

pub struct EncryptedReader<CR: ClientReader>{
    pub(crate) pull_stream: DryocStream<Pull>,
    client_reader: CR,
    buffer: Option<Vec<u8>>
}

impl<CR: ClientReader> EncryptedReader<CR> {

    pub fn new(pull_stream: DryocStream<Pull>, client_reader: CR) -> EncryptedReader<CR> {
        EncryptedReader {client_reader, pull_stream, buffer: None}
    }
}

impl<CR: ClientReader> ClientReader for EncryptedReader<CR> {
    fn try_read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut msg: Vec<u8> = self.buffer.take().unwrap_or(Vec::new());

        loop {
            let block = match self.client_reader.try_read() {
                Ok(block) => block,
                Err(e) => {
                    if msg.len() > 0 {
                        self.buffer = Some(msg);
                    }

                    return Err(e);
                }
            };

            let (decrypted_block, tag) = self.pull_stream.pull_to_vec(&block, None)?;

            msg.extend_from_slice(decrypted_block.as_slice());

            if tag == Tag::PUSH {
                return Ok(msg);
            }
        }
    }

    fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut msg: Vec<u8> = self.buffer.take().unwrap_or(Vec::new());

        loop {
            let block = match self.client_reader.read(timeout) {
                Ok(block) => block,
                Err(e) => {
                    if msg.len() > 0 {
                        self.buffer = Some(msg);
                    }

                    return Err(e);
                }
            };

            let (decrypted_block, tag) = self.pull_stream.pull_to_vec(&block, None)?;

            msg.extend_from_slice(decrypted_block.as_slice());

            if tag == Tag::PUSH {
                return Ok(msg);
            }
        }
    }
}

pub struct EncryptedWriter<CW: ClientWriter> {
    pub(crate) push_stream: DryocStream<Push>,
    client_writer: CW
}

impl<CW: ClientWriter> EncryptedWriter<CW> {

    pub fn new(push_stream: DryocStream<Push>, client_writer: CW) -> EncryptedWriter<CW> {
        EncryptedWriter {client_writer, push_stream}
    }

}

impl<CW: ClientWriter> ClientWriter for EncryptedWriter<CW> {
    fn write(&mut self, msg: &[u8]) -> Result<(), Box<dyn Error>> {
        for i in (BLOCK_SIZE..=msg.len()).step_by(BLOCK_SIZE) {
            let block = &msg[i - BLOCK_SIZE..i];

            let mut tag = Tag::MESSAGE;
            if i == msg.len() {
                tag = Tag::PUSH;
            }

            let encrypted_block = self.push_stream.push_to_vec(&block, None, tag)?;

            self.client_writer.write(&encrypted_block)?;
        }

        if msg.len() % BLOCK_SIZE != 0 {
            let block = &msg[msg.len()-(msg.len()%BLOCK_SIZE)..msg.len()];

            let encrypted_block = self.push_stream.push_to_vec(&block, None, Tag::PUSH)?;

            self.client_writer.write(&encrypted_block)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use crate::protocol::{Active, Connection, Encrypted, Udp, Waiting};
    use super::*;

    fn connect() -> (Connection<Active<Encrypted<Udp>>>, Connection<Active<Encrypted<Udp>>>){
        let timeout = Duration::from_millis(100);

        let c1 = Connection::<Waiting>::new(None).unwrap();
        let c2 = Connection::<Waiting>::new(None).unwrap();

        let p1 = c1.get_port();
        let p2 = c2.get_port();

        let ipv6 = Ipv6Addr::from(1);

        let thread_c2 = thread::spawn(move || {
            let c2 = c2.connect(ipv6, p1, Some(timeout), Some(timeout)).unwrap();
            return c2.encrypt().unwrap();
        });

        let c1 = c1.connect(ipv6, p2, Some(timeout), Some(timeout)).unwrap();
        let mut c1 = c1.encrypt().unwrap();
        let c2 = thread_c2.join().unwrap();

        (c1, c2)
    }

    #[test]
    fn test_one_block() {
        let (c1, c2) = connect();

        let (mut c1_writer, mut c1_reader) = c1.accept();
        let (mut c2_writer, mut c2_reader) = c2.accept();

        let c1_msg = b"Das ist ein Test. Diese Nachricht wird von c1 an c2 versendet.";
        let c2_msg = b"Das ist ein Test. Diese Nachricht wird von c2 an c1 versendet.";

        c1_writer.write(c1_msg.as_slice()).unwrap();
        c2_writer.write(c2_msg.as_slice()).unwrap();

        let c1_recv = c1_reader.try_read().unwrap();
        let c2_recv = c2_reader.try_read().unwrap();

        assert_eq!(c1_msg.as_slice(), c2_recv.as_slice());
        assert_eq!(c2_msg.as_slice(), c1_recv.as_slice());
    }

    #[test]
    fn test_multi_block() {
        let (mut c1, mut c2) = connect();

        let (mut c1_writer, mut c1_reader) = c1.accept();
        let (mut c2_writer, mut c2_reader) = c2.accept();

        let fitting_msg = [24; BLOCK_SIZE*3];
        let overflow_msg = [24; BLOCK_SIZE*3+BLOCK_SIZE/3];

        c1_writer.write(fitting_msg.as_slice()).unwrap();
        c2_writer.write(overflow_msg.as_slice()).unwrap();

        let c1_recv = c1_reader.try_read().unwrap();
        let c2_recv = c2_reader.try_read().unwrap();

        assert_eq!(fitting_msg.as_slice(), c2_recv.as_slice());
        assert_eq!(overflow_msg.as_slice(), c1_recv.as_slice());
    }

    #[test]
    fn try_read_async() {
        let (c1, c2) = connect();

        let (mut c1_writer, _c1_reader) = c1.accept();
        let (mut _c2_writer, mut c2_reader) = c2.accept();

        let overflow_msg = [24; BLOCK_SIZE*100+BLOCK_SIZE/3];

        let thread_c1 = thread::spawn(move || {
            c1_writer.write(&overflow_msg).unwrap();
        });

        let thread_c2 = thread::spawn(move || {
            let res = c2_reader.try_read().unwrap_or(vec![0]);
            return (c2_reader, res);
        });

        thread_c1.join().unwrap();
        let (mut c2_reader, mut overflow_recv) = thread_c2.join().unwrap();

        if overflow_msg.as_slice() != overflow_recv.as_slice() {
            overflow_recv = c2_reader.try_read().unwrap();
        }

        assert_eq!(overflow_msg.as_slice(), overflow_recv.as_slice());
    }
}
