use crate::client::{ActiveClient, ClientReader, ClientWriter};
use crate::error::{ChangeStateError, ErrorKind};
use std::error::Error;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};
use crate::error::{Error as P2pError};

const MSG_RESEND_DELAY: Duration = Duration::from_millis(100);
const PING_RESEND_DELAY: Duration = Duration::from_millis(50);

pub struct UdpWaitingClient {
    udp_socket: UdpSocket,
}

impl UdpWaitingClient {
    pub fn new(port: Option<u16>) -> Result<UdpWaitingClient, P2pError> {
        let bind_addr = IpAddr::from(Ipv6Addr::from(0));
        let bind_addr = SocketAddr::new(bind_addr, port.unwrap_or(0));
        let udp_socket = UdpSocket::bind(&bind_addr)?;

        Ok(UdpWaitingClient { udp_socket })
    }

    pub fn connect(
        self,
        peer: Ipv6Addr,
        port: u16,
        connect_timeout: Option<Duration>,
        disconnect_timeout: Option<Duration>,
    ) -> Result<UdpActiveClient, ChangeStateError<Self>> {
        let peer_addr = IpAddr::from(peer);
        let peer_addr = SocketAddr::new(peer_addr, port);

        if self.get_port() == port {
            return Err(ChangeStateError::new(self, Box::new(P2pError::new(ErrorKind::CannotConnectToSelf))));
        }

        match self.udp_socket.connect(&peer_addr) {
            Ok(_) => {}
            Err(e) => return Err(ChangeStateError::new(self, Box::new(e))),
        }

        let udp_socket_copy = match self.udp_socket.try_clone() {
            Ok(socket) => socket,
            Err(e) => return Err(ChangeStateError::new(self, Box::new(e))),
        };

        // program should panic if this fails
        let mut active_client = UdpActiveClient::new(self.udp_socket, disconnect_timeout).unwrap();

        match active_client.writer_ref().ping(connect_timeout) {
            Ok(_) => {}
            Err(e) => {
                println!("{}", e);
                return Err(ChangeStateError::new(
                    UdpWaitingClient {
                        udp_socket: udp_socket_copy,
                    },
                    Box::new(e),
                ))
            }
        }

        return Ok(active_client);
    }

    pub fn get_port(&self) -> u16 {
        // program should panic if this fails
        self.udp_socket.local_addr().unwrap().port()
    }
}

pub struct UdpActiveClient {
    writer_client: UdpClientWriter,
    reader_client: UdpClientReader,
}

pub struct UdpClientReader {
    thread_handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    stop_thread: Sender<()>,
    message_receiver: Receiver<Vec<u8>>,
}

pub struct UdpClientWriter {
    udp_socket: UdpSocket,
    send_counter: u8,
    ack_receiver: Receiver<u8>,
    timeout: Duration,
}

impl UdpClientReader {
    pub fn new(
        udp_socket: UdpSocket,
        ack_sender: Sender<u8>,
    ) -> Result<UdpClientReader, P2pError> {
        let (stop_sender, stop_receiver) = mpsc::channel::<()>();
        let (message_sender, message_receiver) = mpsc::channel::<Vec<u8>>();
        udp_socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        let thread_handle: JoinHandle<Result<(), Box<dyn Error + Send + Sync>>> =
            thread::spawn(move || {
                let mut instant = Instant::now();
                let mut msg_counter = 0;

                udp_socket.set_nonblocking(false)?;

                loop {
                    if stop_receiver.try_recv().is_ok() {
                        println!("shutting down");
                        return Ok(());
                    }

                    let mut header = [0u8; 6];

                    match udp_socket.peek(header.as_mut_slice()) {
                        Ok(_) => {}
                        Err(_e) => {
                            if instant.elapsed() > Duration::from_secs(1) {
                                return Ok(());
                            }

                            if instant.elapsed() > Duration::from_millis(100){
                                println!("recv err: {}", _e);
                                udp_socket.send([0xCA, 0x00].as_slice())?;
                                sleep(Duration::from_millis(5));
                                instant = Instant::now();
                            }

                            if header[0] == 0 {
                                continue;
                            }
                        }
                    }

                    let msg_type = header[0];
                    let msg_number = header[1];

                    match msg_type {
                        0xAA => {
                            udp_socket.recv(header.as_mut_slice())?;

                            //println!("AA {}", msg_number);

                            ack_sender.send(msg_number)?;
                        }
                        0xDD => {
                            let msg_len = u32::from_be_bytes(header[2..6].try_into()?);

                            let mut msg_content = vec![0u8; msg_len as usize + 6];
                            for _ in 0..msg_len {
                                msg_content.push(0);
                            }

                            let _actual_len = match udp_socket.recv(msg_content.as_mut_slice()) {
                                Ok(l) => l as u32,
                                Err(_) => continue,
                            };

                            let msg_content = Vec::from(&msg_content[6..msg_len as usize + 6]);

                            /*println!(
                                "DD {}: l({}) v({}) - {:?}",
                                msg_number,
                                msg_len,
                                msg_content.len(),
                                msg_content.as_slice()
                            );*/

                            if msg_number == msg_counter {
                                msg_counter = match msg_counter {
                                    255 => 0,
                                    x => x + 1,
                                };

                                message_sender.send(msg_content)?;
                            }

                            udp_socket.send([0xAA, msg_number].as_slice())?;
                        }
                        0xCA => {
                            udp_socket.recv(header.as_mut_slice())?;
                            println!("CA");
                        }
                        0xCC => {
                            udp_socket.recv(header.as_mut_slice())?;

                            println!("CC {}", msg_number);

                            if msg_number == msg_counter {
                                msg_counter = match msg_counter {
                                    255 => 0,
                                    x => x + 1,
                                };
                            }

                            udp_socket.send([0xAA, msg_number].as_slice())?;
                        }
                        _ => continue,
                    }
                }
            });

        return Ok(UdpClientReader {
            message_receiver,
            thread_handle: Some(thread_handle),
            stop_thread: stop_sender,
        });
    }
}

impl ClientReader for UdpClientReader {
    fn try_read(&mut self) -> Result<Vec<u8>, P2pError> {
        Ok(self.message_receiver.try_recv()?)
    }

    fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, P2pError> {
        return match timeout {
            None => {
                match self.thread_handle.as_ref() {
                    None => return Err(P2pError::new(ErrorKind::TimedOut)),
                    Some(th) => {
                        if th.is_finished() {
                            return Err(P2pError::new(ErrorKind::TimedOut));
                        }
                    },
                }
                Ok(self.message_receiver.recv()?)
            },
            Some(t) => Ok(self.message_receiver.recv_timeout(t)?),
        };
    }
}

impl Drop for UdpClientReader {
    fn drop(&mut self) {
        // #TODO better handling of stop_thread if it fails
        match self.stop_thread.send(()){
            _ => {}
        };
        match self.thread_handle.take().unwrap().join() {
            _ => {}
        };
    }
}

impl UdpClientWriter {
    pub fn new(
        udp_socket: UdpSocket,
        ack_receiver: Receiver<u8>,
        timeout: Option<Duration>,
    ) -> UdpClientWriter {
        return UdpClientWriter {
            udp_socket,
            ack_receiver,
            send_counter: 0,
            timeout: timeout.unwrap_or(Duration::from_secs(0)),
        };
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

    fn ping(&mut self, timeout: Option<Duration>) -> Result<(), P2pError> {
        let now = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(0));

        while timeout.is_zero() || now.elapsed() <= timeout {
            //println!("sending1");
            if self.udp_socket.send(&[0xCC, self.send_counter]).is_err() {
                //#TODO
            };
            //println!("sending2");
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
        return Err(P2pError::new(ErrorKind::TimedOut));
    }
}

impl ClientWriter for UdpClientWriter {
    fn write(&mut self, msg: &[u8]) -> Result<(), P2pError> {
        let now = Instant::now();
        let timeout = self.timeout;
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
        return Err(P2pError::new(ErrorKind::TimedOut));
    }
}

impl UdpActiveClient {
    pub fn new(
        udp_socket: UdpSocket,
        ack_timeout: Option<Duration>,
    ) -> Result<UdpActiveClient, P2pError> {
        let (ack_sender, ack_receiver) = mpsc::channel::<u8>();
        let udp_socket_clone = udp_socket.try_clone()?;

        let reader = UdpClientReader::new(udp_socket, ack_sender)?;
        let writer = UdpClientWriter::new(udp_socket_clone, ack_receiver, ack_timeout);

        return Ok(UdpActiveClient {
            reader_client: reader,
            writer_client: writer,
        });
    }
}

impl ActiveClient for UdpActiveClient {
    type Reader = UdpClientReader;
    type Writer = UdpClientWriter;

    fn split(self) -> (UdpClientWriter, UdpClientReader) {
        (self.writer_client, self.reader_client)
    }

    fn reader_ref(&mut self) -> &mut UdpClientReader {
        &mut self.reader_client
    }

    fn writer_ref(&mut self) -> &mut UdpClientWriter {
        &mut self.writer_client
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    const MAX_LEN: usize = 508u32 as usize;

    #[test]
    fn test_prepare_msg() {
        let socket_addr = SocketAddr::new(IpAddr::from(Ipv6Addr::from(1)), 0);
        let udp_socket = UdpSocket::bind(socket_addr).unwrap();
        udp_socket.connect(socket_addr).unwrap();
        let mut active_client = UdpActiveClient::new(udp_socket, None).unwrap();

        let msg = [1, 2, 3, 4];
        let prepared_msg = active_client.writer_ref().prepare_msg(msg.as_slice());
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
        active_client.writer_ref().send_counter = 25;
        let prepared_msg = active_client.writer_ref().prepare_msg(msg.as_slice());
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
        let w1 = UdpWaitingClient::new(None).unwrap();
        assert!(UdpWaitingClient::new(Some(w1.get_port())).is_err());
    }

    #[test]
    fn test_prepare_local() {
        let (c1, c2) = prepare_local();
        drop(c1);
        drop(c2);
    }

    fn prepare_local() -> (UdpActiveClient, UdpActiveClient) {
        let ipv6 = Ipv6Addr::from(1);
        let timeout = Duration::from_secs(2);
        let w1 = UdpWaitingClient::new(None).unwrap();
        let w2 = UdpWaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        println!("p1: {}, p2: {}", p1, p2);

        let thread_c1 = thread::spawn(move || {
            return w1.connect(ipv6, p2, Some(timeout), Some(timeout)).unwrap();
        });
        let thread_c2 = thread::spawn(move || {
            return w2.connect(ipv6, p1, Some(timeout), Some(timeout)).unwrap();
        });

        let c1 = thread_c1.join().unwrap();
        let c2 = thread_c2.join().unwrap();

        return (c1, c2);
    }

    #[test]
    fn test_async_connect_err() {
        let ipv6 = Ipv6Addr::from(1);
        let timeout = Duration::from_millis(5);
        let w1 = UdpWaitingClient::new(None).unwrap();
        let w2 = UdpWaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        let thread_c2 = thread::spawn(move || {
            sleep(Duration::from_secs(10));
            return w2.connect(ipv6, p1, Some(timeout), Some(timeout)).is_err();
        });

        assert!(w1.connect(ipv6, p2, Some(timeout), Some(timeout)).is_err());
        assert!(thread_c2.join().unwrap());
    }

    #[test]
    fn test_async_connect_ok() {
        let ipv6 = Ipv6Addr::from(1);
        let timeout = Duration::from_millis(10000);
        let w1 = UdpWaitingClient::new(None).unwrap();
        let w2 = UdpWaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        let thread_c2 = thread::spawn(move || {
            sleep(Duration::from_millis(5000));
            println!("start");
            return w2.connect(ipv6, p1, Some(timeout), Some(timeout)).unwrap();
        });

        let res = w1.connect(ipv6, p2, Some(timeout), Some(timeout));
        assert!(res.is_ok());
        drop(thread_c2.join().unwrap());
        drop(res.unwrap());
    }

    #[test]
    fn test_send_local() {
        let (mut c1, mut c2) = prepare_local();

        assert!(c1.writer_ref().write([1, 2, 3, 4].as_slice()).is_ok());
        assert!(c2.writer_ref().write([1, 2, 3, 4].as_slice()).is_ok());

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_read_local() {
        let (mut c1, mut c2) = prepare_local();
        let msg = [1, 2, 3, 4];
        let timeout = Duration::from_secs(2);

        c1.writer_ref().write(msg.as_slice()).unwrap();
        c2.writer_ref().write(msg.as_slice()).unwrap();

        assert_eq!(c1.reader_ref().read(Some(timeout)).unwrap(), msg);
        assert_eq!(c2.reader_ref().read(Some(timeout)).unwrap(), msg);

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_stress_local() {
        let (mut c1, mut c2) = prepare_local();

        for i in 0..10000u32 {
            c1.writer_ref().write(&i.to_be_bytes()).unwrap();
        }

        for i in 0..10000u32 {
            assert_eq!(
                u32::from_be_bytes(
                    c2.reader_ref()
                        .try_read()
                        .unwrap()
                        .as_slice()
                        .try_into()
                        .unwrap()
                ),
                i
            );
        }

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_reader_thread() {
        let (c1, c2) = prepare_local();

        assert!(!c1
            .reader_client
            .thread_handle
            .as_ref()
            .unwrap()
            .is_finished());
        assert!(!c2
            .reader_client
            .thread_handle
            .as_ref()
            .unwrap()
            .is_finished());
    }

    #[test]
    fn test_write_string() {
        let (mut c1, mut c2) = prepare_local();
        let timeout = Duration::from_secs(2);
        let msg = b"Hallo mein Freund! Wie geht es dir?";

        c1.writer_ref().write(msg).unwrap();

        assert_eq!(c2.reader_ref().read(Some(timeout)).unwrap(), msg);
        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_write_max_len() {
        let (mut c1, mut c2) = prepare_local();
        let timeout = Duration::from_secs(2);
        let mut msg: Vec<u8> = Vec::with_capacity(MAX_LEN);

        (0..MAX_LEN).for_each(|i| {
            msg.push((i % 256) as u8);
        });

        c1.writer_ref().write(msg.as_slice()).unwrap();

        assert_eq!(c2.reader_ref().read(Some(timeout)).unwrap(), msg.as_slice());
        drop(c1);
        drop(c2);
    }
}
