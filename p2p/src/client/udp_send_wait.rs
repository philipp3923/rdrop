use std::error::Error;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};

use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};

use crate::client::{ActiveClient, ClientReader, ClientWriter};
use crate::error::Error as P2pError;
use crate::error::{ChangeStateError, ErrorKind};

const SEND_INTERVAL: Duration = Duration::from_millis(70);
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_millis(200); //time between each keep alive message
                                                                  //time between each resend
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);
//time after which the connection is considered dead
const RECEIVE_INTERVAL: Duration = Duration::from_millis(10); //time between each receive timeout

/// A UDP client that waits for a connection.
pub struct UdpWaitingClient {
    udp_socket: UdpSocket,
}

#[repr(u8)]
#[derive(Debug)]
enum MessageType {
    Open = 0x01,
    Data = 0x02,
    Acknowledge = 0x03,
    KeepAlive = 0x04,
    Invalid = 0x05,
}

impl From<u8> for MessageType {
    fn from(byte: u8) -> Self {
        match byte {
            0x01 => MessageType::Open,
            0x02 => MessageType::Data,
            0x03 => MessageType::Acknowledge,
            0x04 => MessageType::KeepAlive,
            _ => MessageType::Invalid,
        }
    }
}

impl UdpWaitingClient {
    /// Creates a new `UdpWaitingClient`.
    ///
    /// # Examples
    ///
    /// ```
    /// use p2p::client::udp_slide::UdpWaitingClient;
    ///
    /// let client = UdpWaitingClient::new(None);
    ///
    /// match client {
    ///     Ok(client) => {
    ///         println!("Client created");
    ///     }
    ///     Err(err) => {
    ///         println!("Error: {}", err);
    ///     }
    /// }
    /// ```
    /// # Arguments
    ///
    /// * `port` - An optional `u16` value representing the port to bind to. If `None` is provided,
    ///             a random port will be chosen.
    ///
    /// # Returns
    ///
    /// Returns a `Result` that contains a `UdpWaitingClient` instance if successful, or a `P2pError` if an error occurs during socket binding.
    pub fn new(port: Option<u16>) -> Result<UdpWaitingClient, P2pError> {
        let bind_addr = IpAddr::from(Ipv6Addr::from(0));
        let bind_addr = SocketAddr::new(bind_addr, port.unwrap_or(0));
        let udp_socket = UdpSocket::bind(&bind_addr)?;

        // clear the udp buffer
        udp_socket.set_read_timeout(Some(RECEIVE_INTERVAL))?;
        let mut buf = [0; 1];
        while udp_socket.recv(&mut buf).is_ok() && buf[0] != MessageType::Open as u8 {}

        Ok(UdpWaitingClient { udp_socket })
    }

    /// Connects to a peer and transitions to an active client state.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use p2p::client::udp_slide::UdpWaitingClient;
    ///
    /// let client = UdpWaitingClient::new(None).unwrap();
    ///
    /// let active_client = client.connect("0:0:0:0:0:0:0:1".parse().unwrap(), 9000, Some(Duration::from_millis(1)), None);
    ///
    /// match active_client {
    ///     Ok(client) => {
    ///         println!("Connected to peer");
    ///     }
    ///     Err(err) => {
    ///         println!("Error: {}", err);
    ///     }
    /// }
    /// ```
    /// # Arguments
    ///
    /// * `peer` - An `Ipv6Addr` representing the IP address of the peer.
    /// * `port` - A `u16` value representing the port of the peer.
    /// * `connect_timeout` - An optional `Duration` specifying the maximum time to wait for the connection to be established.
    /// * `disconnect_timeout` - An optional `Duration` specifying the maximum time to wait after receiving no answer before closing the connection.
    ///
    /// # Returns
    ///
    /// Returns a `Result` that contains an `UdpActiveClient` instance if successful, or a `ChangeStateError` with the previous state and the error which occurred.
    pub fn connect(
        mut self,
        peer: Ipv6Addr,
        port: u16,
        connect_timeout: Option<Duration>,
        disconnect_timeout: Option<Duration>,
    ) -> Result<UdpActiveClient, ChangeStateError<Self>> {
        let peer_addr = IpAddr::from(peer);
        let peer_addr = SocketAddr::new(peer_addr, port);

        if self.get_port() == port {
            return Err(ChangeStateError::new(
                self,
                Box::new(P2pError::new(ErrorKind::CannotConnectToSelf)),
            ));
        }

        match self.udp_socket.connect(&peer_addr) {
            Ok(_) => {}
            Err(e) => return Err(ChangeStateError::new(self, Box::new(e))),
        }

        if let Err(e) = self.ping_and_wait(connect_timeout) {
            return Err(ChangeStateError::new(self, Box::new(e)));
        };

        // program should panic if this fails
        let active_client = UdpActiveClient::new(self.udp_socket, disconnect_timeout).unwrap();

        return Ok(active_client);
    }

    fn ping_and_wait(&mut self, timeout: Option<Duration>) -> Result<(), P2pError> {
        self.udp_socket.set_read_timeout(Some(RECEIVE_INTERVAL))?;
        let timeout = timeout.unwrap_or(Duration::from_secs(0));
        let udp_socket_clone = self.udp_socket.try_clone()?;
        let (stop_send, stop_receive) = channel::<()>();

        let receive_thread = thread::spawn(move || {
            let mut buf = [0; 1];

            while buf[0] != MessageType::Open as u8 && stop_receive.try_recv().is_err() {
                let _ = udp_socket_clone.recv(&mut buf);
            }
        });

        let now = Instant::now();

        while !receive_thread.is_finished() {
            self.udp_socket.send(&[MessageType::Open as u8])?;
            sleep(RECEIVE_INTERVAL);

            if now.elapsed() > timeout {
                stop_send.send(())?;
                let _ = receive_thread.join();
                return Err(P2pError::new(ErrorKind::TimedOut));
            }
        }
        self.udp_socket.send(&[MessageType::Open as u8])?;

        let mut buf = [0; 1];
        while buf[0] == MessageType::Open as u8 && self.udp_socket.recv(&mut buf).is_ok() {}

        Ok(())
    }

    /// Returns the local port number that the client is bound to.
    ///
    /// # Examples
    ///
    /// ```
    /// use p2p::client::udp_slide::UdpWaitingClient;
    ///
    /// let client = UdpWaitingClient::new(None).unwrap();
    ///
    /// let port = client.get_port();
    ///
    /// println!("Local port: {}", port);
    /// ```
    ///
    /// # Returns
    ///
    /// Returns a `u16` value representing the local port number.
    ///
    /// # Panics
    ///
    /// The program will panic if obtaining the local port number fails.
    pub fn get_port(&self) -> u16 {
        // program should panic if this fails
        self.udp_socket.local_addr().unwrap().port()
    }
}

/// An active UDP client.
pub struct UdpActiveClient {
    writer_client: UdpClientWriter,
    reader_client: UdpClientReader,
}

/// Reader part of the UDP client.
pub struct UdpClientReader {
    thread_handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    stop_thread: Sender<()>,
    message_receiver: Receiver<Vec<u8>>,
}

/// Writer part of the UDP client.
pub struct UdpClientWriter {
    udp_socket: UdpSocket,
    send_counter: u8,
    ack_receiver: Receiver<u8>,
    timeout: Duration,
    closed_receiver: Receiver<()>,
}

impl UdpClientReader {
    /// Creates a new `UdpClientReader`.
    ///
    /// # Arguments
    ///
    /// * `udp_socket` - A `UdpSocket` representing the UDP socket to read from.
    /// * `ack_sender` - A `Sender<u8>` used for sending acknowledgments to the sender part.
    /// * `closed_sender` - A `Sender<()>` for notifying the thread that the connection has been closed.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the `UdpClientReader` instance if successful, or a `P2pError` if an error occurs.
    fn new(
        udp_socket: UdpSocket,
        ack_sender: Sender<u8>,
        closed_sender: Sender<()>,
    ) -> Result<UdpClientReader, P2pError> {
        let (stop_sender, stop_receiver) = channel::<()>();
        let (message_sender, message_receiver) = channel::<Vec<u8>>();
        udp_socket.set_read_timeout(Some(RECEIVE_INTERVAL))?;
        udp_socket.set_nonblocking(false)?;

        let thread_handle: JoinHandle<Result<(), Box<dyn Error + Send + Sync>>> =
            thread::spawn(move || {
                UdpClientReader::read_thread(
                    udp_socket,
                    stop_receiver,
                    ack_sender,
                    closed_sender,
                    message_sender,
                )
            });

        return Ok(UdpClientReader {
            message_receiver,
            thread_handle: Some(thread_handle),
            stop_thread: stop_sender,
        });
    }

    /// Reads messages from a UDP socket.
    ///
    /// # Arguments
    ///
    /// * `udp_socket` - A `UdpSocket` representing the UDP socket to read from.
    /// * `stop_receiver` - A `Receiver<()>` used for receiving a stop signal to terminate the thread.
    /// * `ack_sender` - A `Sender<u8>` used for sending acknowledgments to the sender part.
    /// * `message_sender` - A `Sender<Vec<u8>>` used for sending the received message content.
    ///
    /// # Returns
    ///
    /// Returns a Result containing `Ok(())` if the method terminates successfully or a `Box<dyn Error + Send + Sync>` if it fails.
    fn read_thread(
        udp_socket: UdpSocket,
        stop_receiver: Receiver<()>,
        ack_sender: Sender<u8>,
        closed_sender: Sender<()>,
        message_sender: Sender<Vec<u8>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut keep_alive_time = Instant::now();
        let mut dead_time = Instant::now();
        let mut msg_counter = 0;
        let mut opening = true;
        udp_socket.set_read_timeout(Some(RECEIVE_INTERVAL))?;

        println!("[UDP] receive thread started");

        loop {
            if keep_alive_time.elapsed() > KEEP_ALIVE_INTERVAL {
                udp_socket.send(&[MessageType::KeepAlive as u8])?;
                keep_alive_time = Instant::now();
                //println!("[UDP] send keep alive");
            }

            if stop_receiver.try_recv().is_ok() {
                println!("[UDP] read thread stopped");
                closed_sender.send(())?;
                return Ok(());
            }

            let mut header = [0u8; 6];

            match udp_socket.peek(header.as_mut_slice()) {
                Ok(_) => {
                    if opening && header[0] != MessageType::Open as u8 {
                        opening = false;
                    }
                }
                Err(_e) => {
                    if dead_time.elapsed() > DISCONNECT_TIMEOUT {
                        println!("[UDP] read thread timeout");
                        closed_sender.send(())?;
                        return Ok(());
                    }
                }
            }

            //println!("[UDP] received message type: {:?}", MessageType::from(header[0]));
            if header[0] != 0 {
                dead_time = Instant::now();
            } else {
                sleep(RECEIVE_INTERVAL);
                continue;
            }

            let msg_type = header[0];
            let msg_number = header[1];

            match msg_type.into() {
                MessageType::Open => {
                    udp_socket.recv(header.as_mut_slice())?;
                    if opening {
                        continue;
                    }
                    println!("[UDP] received open message.. shutting down");
                    closed_sender.send(())?;
                    return Ok(());
                }
                MessageType::Acknowledge => {
                    udp_socket.recv(header.as_mut_slice())?;
                    ack_sender.send(msg_number)?;
                }
                MessageType::Data => {
                    let msg_len = u32::from_be_bytes(header[2..6].try_into()?);

                    let mut msg_content = vec![0u8; msg_len as usize + 6];
                    for _ in 0..msg_len + 6 {
                        msg_content.push(0);
                    }

                    let _actual_len = match udp_socket.recv(msg_content.as_mut_slice()) {
                        Ok(l) => l as u32,
                        Err(_) => continue,
                    };

                    let msg_content = Vec::from(&msg_content[6..msg_len as usize + 6]);

                    if msg_number == msg_counter {
                        msg_counter = match msg_counter {
                            255 => 0,
                            x => x + 1,
                        };

                        message_sender.send(msg_content)?;
                    }

                    udp_socket.send([MessageType::Acknowledge as u8, msg_number].as_slice())?;
                }
                MessageType::KeepAlive => {
                    udp_socket.recv(header.as_mut_slice())?;
                }
                MessageType::Invalid => {
                    udp_socket.recv(header.as_mut_slice())?;
                    println!("[UDP] received invalid msg {}", msg_type);
                    continue;
                }
            }
        }
    }

    fn validate_thread_handle(&self) -> Result<(), P2pError> {
        if let Some(thread_handle) = self.thread_handle.as_ref() {
            if thread_handle.is_finished() {
                return Err(P2pError::new(ErrorKind::CommunicationFailed));
            }
        } else {
            return Err(P2pError::new(ErrorKind::CommunicationFailed));
        }
        Ok(())
    }
}

impl ClientReader for UdpClientReader {
    /// Tries to read a message from the `UdpClientReader` message receiver.
    ///
    /// # Returns
    ///
    /// Returns a Result containing a `Vec<u8>` with the received message content if available, or a `P2pError` if no message is available or the connection was closed.
    fn try_read(&mut self) -> Result<Vec<u8>, P2pError> {
        self.validate_thread_handle()?;

        Ok(self.message_receiver.try_recv()?)
    }

    /// Reads a message from the `UdpClientReader` message receiver.
    ///
    /// # Arguments
    ///
    /// * `timeout` - An optional `Duration` indicating the maximum time to wait for a message. If `None` is passed, the method will block until a message is received.
    ///
    /// # Returns
    ///
    /// Returns a Result containing a `Vec<u8>` with the received message content if available, or a `P2pError` if no message is available or the connection was closed.
    fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, P2pError> {
        self.validate_thread_handle()?;

        return match timeout {
            None => Ok(self.message_receiver.recv()?),
            Some(t) => Ok(self.message_receiver.recv_timeout(t)?),
        };
    }
}

impl Drop for UdpClientReader {
    fn drop(&mut self) {
        if let Err(err) = self.stop_thread.send(()) {
            println!(
                "Error occurred when trying to stop the reader thread: {:?}",
                err
            );
        }

        if let Some(thread_handle) = self.thread_handle.take() {
            if let Err(err) = thread_handle.join() {
                println!("Error occurred when joining the reader thread: {:?}", err);
            }
        }
    }
}

impl UdpClientWriter {
    /// Creates a new `UdpClientWriter`.
    ///
    /// # Arguments
    ///
    /// * `udp_socket` - A `UdpSocket`  to write to.
    /// * `ack_receiver` - A `Receiver<u8>` used for receiving acknowledgment messages.
    /// * `timeout` - An optional `Duration` indicating the maximum time to wait for acknowledgments. If `None` is passed, the method will block until an acknowledgment is received.
    /// * `closed_receiver` - A `Receiver<()>` used for receiving notifications that the client is closed.
    ///
    /// # Returns
    ///
    /// Returns an `UdpClientWriter`.
    fn new(
        udp_socket: UdpSocket,
        ack_receiver: Receiver<u8>,
        timeout: Option<Duration>,
        closed_receiver: Receiver<()>,
    ) -> UdpClientWriter {
        return UdpClientWriter {
            udp_socket,
            ack_receiver,
            send_counter: 0,
            timeout: timeout.unwrap_or(Duration::from_secs(0)),
            closed_receiver,
        };
    }

    fn prepare_msg(&mut self, msg: &[u8]) -> Vec<u8> {
        let len = msg.len();
        let mut result = Vec::with_capacity(len + 4 + 4);

        result.push(MessageType::Data as u8);
        result.push(self.send_counter);
        result.extend_from_slice(&(len as u32).to_be_bytes());
        result.extend_from_slice(msg);

        result
    }
}

impl ClientWriter for UdpClientWriter {
    /// Writes a message to the UDP socket.
    ///
    /// # Arguments
    ///
    /// * `msg` - A slice of `u8` representing the message to be sent.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the message is successfully sent and acknowledged or a `P2pError` if an error occurs or the operation times out.
    fn write(&mut self, msg: &[u8]) -> Result<(), P2pError> {
        let now = Instant::now();
        let timeout = self.timeout;
        let msg = self.prepare_msg(msg);

        while timeout.is_zero() || now.elapsed() <= timeout {
            if self.closed_receiver.try_recv().is_ok() {
                return Err(P2pError::new(ErrorKind::CommunicationFailed));
            }

            self.udp_socket
                .send(msg.as_slice())
                .map_err(|_| P2pError::new(ErrorKind::CommunicationFailed))?;

            match self.ack_receiver.recv_timeout(SEND_INTERVAL) {
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
        println!("[UDP] send timeout");
        return Err(P2pError::new(ErrorKind::TimedOut));
    }
}

impl UdpActiveClient {
    /// Creates a new `UdpActiveClient`.
    ///
    /// # Arguments
    ///
    /// * `udp_socket` - A `UdpSocket` for communication.
    /// * `ack_timeout` - An optional `Duration` indicating the maximum time to wait for acknowledgments. If `None` is provided, the connection wont time out.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the `UdpActiveClient` instance if it is successfully created, or a `P2pError` that occurred during initialization.
    pub fn new(
        udp_socket: UdpSocket,
        ack_timeout: Option<Duration>,
    ) -> Result<UdpActiveClient, P2pError> {
        let (ack_sender, ack_receiver) = channel::<u8>();
        let udp_socket_clone = udp_socket.try_clone()?;

        let (closed_writer, closed_receiver) = channel::<()>();

        let reader = UdpClientReader::new(udp_socket, ack_sender, closed_writer)?;
        let writer =
            UdpClientWriter::new(udp_socket_clone, ack_receiver, ack_timeout, closed_receiver);

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
    use std::thread::sleep;

    use super::*;

    const MAX_LEN: usize = 508u32 as usize;

    #[test]
    fn test_prepare_msg() {
        let socket_addr = SocketAddr::new(IpAddr::from(Ipv6Addr::from(1)), 0);
        let udp_socket = UdpSocket::bind(socket_addr).unwrap();
        udp_socket.connect(socket_addr).unwrap();
        let mut active_client = UdpActiveClient::new(udp_socket, None).unwrap();

        let msg = [1, 2, 3, 4];
        let prepared_msg = active_client.writer_ref().prepare_msg(msg.as_slice());
        assert_eq!(prepared_msg[0], MessageType::Data as u8);
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
        assert_eq!(prepared_msg[0], MessageType::Data as u8);
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
        let timeout = Duration::from_millis(1);
        let w1 = UdpWaitingClient::new(None).unwrap();
        let w2 = UdpWaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        let thread_c2 = thread::spawn(move || {
            sleep(Duration::from_millis(500));
            return w2.connect(ipv6, p1, Some(timeout), Some(timeout)).is_err();
        });

        assert!(w1.connect(ipv6, p2, Some(timeout), Some(timeout)).is_err());
        assert!(thread_c2.join().unwrap());
    }

    #[test]
    fn test_async_connect_ok() {
        let ipv6 = Ipv6Addr::from(1);
        let timeout = Duration::from_millis(1000);
        let w1 = UdpWaitingClient::new(None).unwrap();
        let w2 = UdpWaitingClient::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

        let thread_c2 = thread::spawn(move || {
            sleep(Duration::from_millis(100));
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
