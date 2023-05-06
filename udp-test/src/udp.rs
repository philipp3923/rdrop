use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::{fmt, thread};
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};

const MSG_RESEND_DELAY: Duration = Duration::from_millis(100);
const PING_RESEND_DELAY: Duration = Duration::from_millis(50);

pub struct WaitingClient {
    udp_socket: UdpSocket,
}

impl WaitingClient {
    pub fn new(port: Option<u16>) -> Result<WaitingClient, Box<dyn Error>> {
        let bind_addr = IpAddr::from(Ipv6Addr::from(0));
        let bind_addr = SocketAddr::new(bind_addr, port.unwrap_or(0));
        let udp_socket = UdpSocket::bind(&bind_addr)?;

        Ok(WaitingClient { udp_socket })
    }

    pub fn connect(
        self,
        peer: Ipv6Addr,
        port: u16,
        timeout: Option<Duration>,
    ) -> Result<ActiveClient, Box<dyn Error>> {
        let peer_addr = IpAddr::from(peer);
        let peer_addr = SocketAddr::new(peer_addr, port);

        self.udp_socket.connect(&peer_addr)?;

        let mut active_client = ActiveClient::new(self.udp_socket).unwrap();

        active_client.writer_ref().ping(timeout)?;

        return Ok(active_client);
    }

    pub fn get_port(&self) -> u16 {
        self.udp_socket.local_addr().unwrap().port()
    }
}

#[derive(Clone, Debug)]
pub struct TimeoutError(Duration);

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

#[derive(Clone, Debug)]
pub struct MessageOverflowError{
    is: usize,
    should: usize
}

impl MessageOverflowError {
    fn new(is: usize, should: usize) -> MessageOverflowError {
        MessageOverflowError {is, should}
    }
}

impl Display for MessageOverflowError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "message is too long. {} instead of the maximum {}", self.is, self.should)
    }
}

impl Error for MessageOverflowError {
    fn description(&self) -> &str {
        "The message is too long"
    }
}


pub struct ActiveClient {
    writer_client: WriterClient,
    reader_client: ReaderClient,
}

pub struct ReaderClient {
    thread_handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    stop_thread: Sender<()>,
    message_receiver: Receiver<Vec<u8>>,
}

pub struct WriterClient {
    udp_socket: UdpSocket,
    send_counter: u8,
    ack_receiver: Receiver<u8>,
}

impl ReaderClient {
    pub fn new(udp_socket: UdpSocket, ack_sender: Sender<u8>) -> Result<ReaderClient, Box<dyn Error>> {
        let (stop_sender, stop_receiver) = mpsc::channel::<()>();
        let (message_sender, message_receiver) = mpsc::channel::<Vec<u8>>();

        udp_socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        let thread_handle: JoinHandle<Result<(), Box<dyn Error + Send + Sync>>> = thread::spawn(move || {
            let mut msg_counter = 0;

            loop {
                if stop_receiver.try_recv().is_ok() {
                    println!("shutting down");
                    return Ok(());
                }

                let mut header = [0u8; 6];

                match udp_socket.peek(header.as_mut_slice()) {
                    Ok(_) => {},
                    Err(e) => { if header[0] == 0 {
                        continue; }
                    },
                }

                let msg_type = header[0];
                let msg_number = header[1];

                match msg_type {
                    0xCC => {
                        udp_socket.recv(header.as_mut_slice()).unwrap();

                        println!("CC {}", msg_number);

                        if msg_number == msg_counter {
                            msg_counter = match msg_counter {
                                255 => 0,
                                x => x + 1,
                            };
                        }

                        udp_socket.send([0xAA, msg_number].as_slice())?;
                    }
                    0xAA => {
                        udp_socket.recv(header.as_mut_slice()).unwrap();

                        println!("AA {}", msg_number);

                        ack_sender.send(msg_number)?;
                    }
                    0xDD => {
                        let msg_len = u32::from_be_bytes(header[2..6].try_into().unwrap());

                        let mut msg_content = vec![0u8; msg_len as usize + 6];
                        for _ in 0..msg_len {
                            msg_content.push(0);
                        }

                        let _actual_len = match udp_socket.recv(msg_content.as_mut_slice()) {
                            Ok(l) => l as u32,
                            Err(_) => continue,
                        };

                        let msg_content = Vec::from(&msg_content[6..msg_len as usize + 6]);

                        println!(
                            "DD {}: l({}) v({}) - {:?}",
                            msg_number,
                            msg_len,
                            msg_content.len(),
                            msg_content.as_slice()
                        );

                        if msg_number == msg_counter {
                            msg_counter = match msg_counter {
                                255 => 0,
                                x => x + 1,
                            };

                            message_sender.send(msg_content)?;

                        }

                        udp_socket.send([0xAA, msg_number].as_slice())?;
                    }
                    _ => continue,
                }
            }
        });

        return Ok(ReaderClient {
           message_receiver,
            thread_handle: Some(thread_handle),
            stop_thread: stop_sender
        });
    }

    pub fn try_read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(self.message_receiver.try_recv()?)
    }

    pub fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, Box<dyn Error>> {
        return match timeout {
            None => Ok(self.message_receiver.recv()?),
            Some(t) => Ok(self.message_receiver.recv_timeout(t)?)
        };
    }
}

impl Drop for ReaderClient {
    fn drop(&mut self) {
        self.stop_thread.send(()).unwrap();
        self.thread_handle.take().unwrap().join().ok();
    }
}

impl WriterClient {

    pub fn new(udp_socket: UdpSocket, ack_receiver: Receiver<u8>) -> WriterClient{
        return WriterClient {
            udp_socket,
            ack_receiver,
            send_counter: 0,
        }
    }

    pub fn ping(&mut self, timeout: Option<Duration>) -> Result<(), Box<dyn Error>> {
        let now = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(0));

        while timeout.is_zero() || now.elapsed() <= timeout {
            self.udp_socket.send(&[0xCC, self.send_counter])?;

            match self.ack_receiver.recv_timeout(PING_RESEND_DELAY) {
                Ok(msg_number) => {
                    if msg_number != self.send_counter {
                        continue;
                    }

                    self.send_counter = match self.send_counter {
                        255 => 0,
                        x => x + 1,
                    };
                    println!("ok");
                    return Ok(());
                }
                Err(_) => continue,
            }
        }
        println!("timeout");
        return Err(Box::new(TimeoutError(timeout)));
    }

    pub fn write(&mut self, msg: &[u8], timeout: Option<Duration>) -> Result<(), Box<dyn Error>> {
        if msg.len() > 2 ^ 32 {
            return Err(Box::new(MessageOverflowError::new(msg.len(), 2 ^ 32)));
        }

        let now = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(0));
        let msg = self.prepare_msg(msg);

        while timeout.is_zero() || now.elapsed() <= timeout {
            self.udp_socket.send(msg.as_slice())?;

            match self.ack_receiver.recv_timeout(MSG_RESEND_DELAY) {
                Ok(msg_number) => {
                    if msg_number != self.send_counter {
                        continue;
                    }

                    self.send_counter = match self.send_counter {
                        255 => 0,
                        x => x + 1,
                    };

                    return Ok(());
                }
                Err(_) => continue,
            }
        }
        println!("timeout");
        return Err(Box::new(TimeoutError(timeout)));
    }

    fn prepare_msg(&mut self, msg: &[u8]) -> Vec<u8> {
        let len = msg.len();
        let mut result = Vec::with_capacity(len + 4 + 4);

        result.push(0xDD);
        result.push(self.send_counter);
        result.extend_from_slice(&(len as u32).to_be_bytes());
        result.extend_from_slice(msg);

        result
    }
}



impl ActiveClient {
    pub fn new(udp_socket: UdpSocket) -> Result<ActiveClient, Box<dyn Error>> {
        let (ack_sender, ack_receiver) = mpsc::channel::<u8>();
        let udp_socket_clone = udp_socket.try_clone()?;

        let reader = ReaderClient::new(udp_socket, ack_sender)?;
        let writer = WriterClient::new(udp_socket_clone, ack_receiver);

        return Ok(ActiveClient {
            reader_client: reader,
            writer_client: writer
        });
    }

    pub fn split(self) -> (WriterClient, ReaderClient) {
        (self.writer_client, self.reader_client)
    }

    pub fn reader_ref(&mut self) -> &mut ReaderClient {
        &mut self.reader_client
    }

    pub fn writer_ref(&mut self) -> &mut WriterClient {
        &mut self.writer_client
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_msg() {
        let socket_addr = SocketAddr::new(IpAddr::from(Ipv6Addr::from(1)), 0);
        let udp_socket = UdpSocket::bind(socket_addr).unwrap();
        udp_socket.connect(socket_addr).unwrap();
        let (mut writer, mut reader) = ActiveClient::new(udp_socket).unwrap().split();

        let msg = [1,2,3,4];
        let prepared_msg = writer.prepare_msg(msg.as_slice());
        assert_eq!(prepared_msg[0], 0xDD);
        assert_eq!(prepared_msg[1], 0);
        assert_eq!(prepared_msg[2], 0);
        assert_eq!(prepared_msg[3], 0);
        assert_eq!(prepared_msg[4], 0);
        assert_eq!(prepared_msg[5], 4);
        assert_eq!(prepared_msg[6], 1);
        assert_eq!(prepared_msg[7], 2);
        assert_eq!(prepared_msg[8], 3);
        assert_eq!(prepared_msg[9], 4);
        assert_eq!(prepared_msg.len(), 10);
        writer.send_counter = 25;
        let prepared_msg = writer.prepare_msg(msg.as_slice());
        assert_eq!(prepared_msg[0], 0xDD);
        assert_eq!(prepared_msg[1], 25);
        assert_eq!(prepared_msg[2], 0);
        assert_eq!(prepared_msg[3], 0);
        assert_eq!(prepared_msg[4], 0);
        assert_eq!(prepared_msg[5], 4);
        assert_eq!(prepared_msg[6], 1);
        assert_eq!(prepared_msg[7], 2);
        assert_eq!(prepared_msg[8], 3);
        assert_eq!(prepared_msg[9], 4);
        assert_eq!(prepared_msg.len(), 10);
    }

    #[test]
    fn test_same_port() {
        let w1 = WaitingClient::new(None).unwrap();
        assert!(WaitingClient::new(Some(w1.get_port())).is_err());
    }

    #[test]
    fn test_prepare_local() {
        let (c1, c2) = prepare_local();
        drop(c1);
        drop(c2);
    }

    fn prepare_local() -> (ActiveClient, ActiveClient) {
        let ipv6 = Ipv6Addr::from(1);
        let timeout = Duration::from_secs(2);
        let w1 = WaitingClient::new(None).unwrap();
        let w2 = WaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        println!("p1: {}, p2: {}", p1, p2);

        let thread_c1 = thread::spawn(move || {
            return w1.connect(ipv6, p2, Some(timeout)).unwrap();
        });
        let thread_c2 = thread::spawn(move || {
            return w2.connect(ipv6, p1, Some(timeout)).unwrap();
        });

        let c1 = thread_c1.join().unwrap();
        let c2 = thread_c2.join().unwrap();

        return (c1, c2);
    }

    #[test]
    fn test_async_connect_err() {
        let ipv6 = Ipv6Addr::from(1);
        let timeout = Duration::from_millis(5);
        let w1 = WaitingClient::new(None).unwrap();
        let w2 = WaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        let thread_c2 = thread::spawn(move || {
            sleep(Duration::from_secs(10));
            return w2.connect(ipv6, p1, Some(timeout)).is_err();
        });

        assert!(w1.connect(ipv6, p2, Some(timeout)).is_err());
        assert!(thread_c2.join().unwrap());
    }

    #[test]
    fn test_async_connect_ok() {
        let ipv6 = Ipv6Addr::from(1);
        let timeout = Duration::from_millis(10000);
        let w1 = WaitingClient::new(None).unwrap();
        let w2 = WaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        let thread_c2 = thread::spawn(move || {
            sleep(Duration::from_millis(5000));
            println!("start");
            return w2.connect(ipv6, p1, Some(timeout)).unwrap();
        });

        let res = w1.connect(ipv6, p2, Some(timeout));
        assert!(res.is_ok());
        drop(thread_c2.join().unwrap());
        drop(res.unwrap());
    }

    #[test]
    fn test_send_local() {
        let (mut c1, mut c2) = prepare_local();
        let timeout = Duration::from_secs(2);

        assert!(c1.writer_ref().write([1, 2, 3, 4].as_slice(), Some(timeout)).is_ok());
        assert!(c2.writer_ref().write([1, 2, 3, 4].as_slice(), Some(timeout)).is_ok());

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_read_local() {
        let (mut c1, mut c2) = prepare_local();
        let msg = [1, 2, 3, 4];
        let timeout = Duration::from_secs(2);

        c1.writer_ref().write(msg.as_slice(), Some(timeout)).unwrap();
        c2.writer_ref().write(msg.as_slice(), Some(timeout)).unwrap();

        assert_eq!(c1.reader_ref().read(Some(timeout)).unwrap(), msg);
        assert_eq!(c2.reader_ref().read(Some(timeout)).unwrap(), msg);

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_stress_local() {
        let (mut c1, mut c2) = prepare_local();
        let timeout = Duration::from_secs(2);

        for i in 0..10000u32 {
            c1.writer_ref().write(&i.to_be_bytes(), Some(timeout)).unwrap();
        }

        for i in 0..10000u32 {
            assert_eq!(
                u32::from_be_bytes(c2.reader_ref().try_read().unwrap().as_slice().try_into().unwrap()),
                i
            );
        }

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_reader_thread(){
        let (mut c1, mut c2) = prepare_local();

        assert!(!c1.reader_client.thread_handle.as_ref().unwrap().is_finished());
        assert!(!c2.reader_client.thread_handle.as_ref().unwrap().is_finished());
    }
}
