use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::mpsc::{channel, Receiver, Sender, sync_channel, SyncSender, TrySendError};
use std::thread;
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};

use crate::client::{ActiveClient, ClientReader, ClientWriter};
use crate::error::{ChangeStateError, ErrorKind, ThreadError};
use crate::error::Error as P2pError;

const SEND_INTERVAL: Duration = Duration::from_millis(500);
//time between each resend
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_millis(200);
//time between each keep alive message
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);
//time after which the connection is considered dead
const RECEIVE_INTERVAL: Duration = Duration::from_millis(10); //time between each receive timeout

const SLIDE_WINDOW: u16 = 1000; //number of packets in the slide window

/// A UDP client that waits for a connection.
pub struct UdpWaitingClient {
    udp_socket: UdpSocket,
}

struct Package {
    message_type: MessageType,
    content: Vec<u8>,
    size: u16,
    number: u16,
    timestamp: Instant,
}

impl Package {
    fn new(content: Vec<u8>, size: u16, number: u16, message_type: MessageType) -> Package {
        Package {
            message_type,
            content,
            size,
            number,
            timestamp: Instant::now(),
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
#[derive(PartialEq)]
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
    /// use p2p::client::udp::UdpWaitingClient;
    ///
    /// let client = UdpWaitingClient::new(Some(8080));
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
    /// use p2p::client::udp::UdpWaitingClient;
    ///
    /// let client = UdpWaitingClient::new(Some(8080)).unwrap();
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
            return Err(ChangeStateError::new(
                self,
                Box::new(e),
            ));
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
    /// use p2p::client::udp::UdpWaitingClient;
    ///
    /// let client = UdpWaitingClient::new(Some(8080)).unwrap();
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
    thread_handle: Option<JoinHandle<Result<(), ThreadError>>>,
    stop_thread: Sender<()>,
    message_receiver: Receiver<Vec<u8>>,
}

/// Writer part of the UDP client.
pub struct UdpClientWriter {
    package_sender: SyncSender<Vec<u8>>,
    closed_receiver: Receiver<()>,
    timeout: Option<Duration>,
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
        package_receiver: Receiver<Vec<u8>>,
        closed_sender: Sender<()>,
    ) -> Result<UdpClientReader, P2pError> {
        let (stop_sender, stop_receiver) = channel::<()>();
        let (message_sender, message_receiver) = channel::<Vec<u8>>();
        udp_socket.set_read_timeout(Some(RECEIVE_INTERVAL))?;
        udp_socket.set_nonblocking(false)?;

        let thread_handle: JoinHandle<Result<(), ThreadError>> =
            thread::spawn(move || {
                let mut client_handler = ClientHandler::new(
                    udp_socket,
                    stop_receiver, package_receiver,
                    closed_sender, message_sender,
                );

                client_handler.run()
            });

        return Ok(UdpClientReader {
            message_receiver,
            thread_handle: Some(thread_handle),
            stop_thread: stop_sender,
        });
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

        println!("Dropped UdpClientReader");
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
        package_sender: SyncSender<Vec<u8>>,
        closed_receiver: Receiver<()>,
        timeout: Option<Duration>,
    ) -> UdpClientWriter {
        return UdpClientWriter {
            timeout,
            package_sender,
            closed_receiver,
        };
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
        if msg.len() >= 65536 {
            return Err(P2pError::new(ErrorKind::IllegalByteStream));
        }

        if self.closed_receiver.try_recv().is_ok() {
            return Err(P2pError::new(ErrorKind::CommunicationFailed));
        }

        let now = Instant::now();

        while self.timeout.is_none() || now.elapsed() <= self.timeout.unwrap_or(Duration::from_secs(0)) {
            match self.package_sender.try_send(Vec::from(msg)) {
                Ok(_) => {
                    return Ok(());
                }
                Err(TrySendError::Full(_)) => {
                    sleep(Duration::from_millis(10));
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(P2pError::new(ErrorKind::CommunicationFailed));
                }
            }
        }

        Err(P2pError::new(ErrorKind::TimedOut))
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
        timeout: Option<Duration>,
    ) -> Result<UdpActiveClient, P2pError> {
        let (package_sender, package_receiver) = sync_channel::<Vec<u8>>(SLIDE_WINDOW as usize);

        let (closed_writer, closed_receiver) = channel::<()>();

        let reader = UdpClientReader::new(udp_socket, package_receiver, closed_writer)?;
        let writer =
            UdpClientWriter::new(package_sender, closed_receiver, timeout);

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


struct ClientHandler {
    udp_socket: UdpSocket,
    stop_receiver: Receiver<()>,
    package_receiver: Receiver<Vec<u8>>,
    closed_sender: Sender<()>,
    message_sender: Sender<Vec<u8>>,
    send_counter: u16,
    received_counter: u16,
    message_window: Vec<Package>,
    message_buffer: Vec<(u16, Vec<u8>)>,
}

impl ClientHandler {
    fn new(udp_socket: UdpSocket,
           stop_receiver: Receiver<()>,
           package_receiver: Receiver<Vec<u8>>,
           closed_sender: Sender<()>,
           message_sender: Sender<Vec<u8>>, ) -> ClientHandler {
        ClientHandler {
            message_sender,
            udp_socket,
            stop_receiver,
            package_receiver,
            closed_sender,
            send_counter: 0,
            received_counter: 0,
            message_window: Vec::new(),
            message_buffer: Vec::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), ThreadError> {
        self.udp_socket.set_read_timeout(Some(RECEIVE_INTERVAL))?;
        self.udp_socket.set_nonblocking(false)?;

        let mut keep_alive_time = Instant::now();
        let mut dead_time = Instant::now();
        let mut opening = true;

        loop {
            if keep_alive_time.elapsed() > KEEP_ALIVE_INTERVAL {
                self.udp_socket.send(&[MessageType::KeepAlive as u8])?;
                keep_alive_time = Instant::now();
            }

            if dead_time.elapsed() > DISCONNECT_TIMEOUT {
                println!("[UDP] read thread timeout");
                self.closed_sender.send(())?;
                return Ok(());
            }

            if self.stop_receiver.try_recv().is_ok() {
                println!("[UDP] read thread stopped");
                self.closed_sender.send(())?;
                return Ok(());
            }

            self.send_messages()?;
            self.repeat_messages()?;
            self.read_messages()?;

            let (message_type, message_number, message_size) = match self.peek_header() {
                Some(header) => {
                    //println!("RECEIVED {:?} number: {} size: {}", header.0, header.1, header.2);
                    dead_time = Instant::now();
                    if opening && header.0 != MessageType::Open {
                        opening = false;
                    }
                    header
                }
                None => {
                    sleep(RECEIVE_INTERVAL);
                    continue;
                }
            };

            match message_type {
                MessageType::Open => {
                    self.udp_socket.recv([0; 5].as_mut_slice())?;
                    if opening {
                        continue;
                    }
                    println!("[UDP] received open message.. shutting down");
                    self.closed_sender.send(())?;
                    return Ok(());
                }
                MessageType::Data => {
                    let content = self.recv_data(message_size)?;
                    if self.message_buffer.len() <= SLIDE_WINDOW as usize || message_number <= self.received_counter {
                        if self.message_buffer.iter().find(|x| x.0 == message_number).is_some() {
                            println!("[UDP] received duplicate msg n:{} s:{}", message_number, message_size);
                        }else{
                            self.message_buffer.push((message_number, content));
                        }
                        self.send_acknowledgement(message_number)?;
                    }else{
                        println!("SLIDE WINDOW FULL, {}", self.received_counter);
                    }
                }
                MessageType::Acknowledge => {
                    self.udp_socket.recv([0; 5].as_mut_slice())?;
                    self.acknowledge_package(message_number);
                }
                MessageType::KeepAlive => {
                    self.udp_socket.recv([0; 5].as_mut_slice())?;
                }
                MessageType::Invalid => {
                    self.udp_socket.recv([0; 5].as_mut_slice())?;
                    println!("[UDP] received invalid msg n:{} s:{}", message_number, message_size);
                }
            }
        }
    }

    fn read_messages(&mut self) -> Result<(), P2pError>{
        self.message_buffer.sort_by(|a, b| a.0.cmp(&b.0));
        for i in 0..self.message_buffer.len() {
            if i >= self.message_buffer.len() {
                break;
            }

            let (number, _) = &self.message_buffer[i];

            if number > &self.received_counter {
                break;
            }

            if number == &self.received_counter {
                let (_number, content) = self.message_buffer.remove(i);
                //println!("RECEIVED number: {} content: {:2x?}", _number, content.as_slice());
                self.message_sender.send(content)?;
                self.received_counter = self.received_counter.wrapping_add(1);
            }


        }

        Ok(())
    }

    fn acknowledge_package(&mut self, message_number: u16) {
        if let Some(index) = self.message_window.iter().position(|package| &&package.number == &&message_number) {
            self.message_window.remove(index);
        }
    }

    fn send_acknowledgement(&mut self, message_number: u16) -> Result<(), P2pError> {
        let message = ClientHandler::encode_msg([0].as_slice(), MessageType::Acknowledge, message_number);
        self.udp_socket.send(message.0.as_slice())?;
        Ok(())
    }

    fn recv_data(&mut self, message_size: u16) -> Result<Vec<u8>, P2pError> {
        let mut buffer = vec![0u8; message_size as usize + 5];
        self.udp_socket.recv(&mut buffer)?;

        //println!("DATA {:2x?}", buffer.as_slice());

        buffer = buffer[5..].to_vec();

        //println!("DATA {:2x?}", buffer.as_slice());
        Ok(buffer)
    }

    fn peek_header(&mut self) -> Option<(MessageType, u16, u16)> {
        let mut header = [0u8; 5];
        if let Err(_e) = self.udp_socket.peek(&mut header) {
            return None;
        };

        if header[0] == 0 {
            return None;
        }

        let header = ClientHandler::decode_header(header);
        Some(header)
    }

    fn repeat_messages(&mut self) -> Result<(), P2pError> {
        self.message_window.iter().rev().for_each(|package| {
            if package.timestamp.elapsed() > SEND_INTERVAL {
                if let Err(e) = self.udp_socket.send(package.content.as_slice()) {
                    println!("[UDP] send error: {:?}", e);
                }
            }
        });

        Ok(())
    }

    fn send_messages(&mut self) -> Result<(), P2pError> {
        loop {
            if self.message_window.len() >= SLIDE_WINDOW as usize {
                break;
            }

            match self.package_receiver.try_recv() {
                Ok(content) => {
                    let (content, size) = ClientHandler::encode_msg(&content, MessageType::Data, self.send_counter);
                    //println!("SEND number: {} size: {} content {:2x?}", self.send_counter, size, content);
                    self.udp_socket.send(content.as_slice())?;
                    self.message_window.push(Package::new(content, size, self.send_counter, MessageType::Data));
                    self.send_counter = self.send_counter.wrapping_add(1);
                }
                Err(_) => break,
            }
        }
        Ok(())
    }

    fn encode_msg(msg: &[u8], message_type: MessageType, message_number: u16) -> (Vec<u8>, u16) {
        let len = msg.len();
        let mut result = Vec::with_capacity(len + 4 + 4);

        result.push(message_type as u8);
        result.extend_from_slice(message_number.to_be_bytes().as_slice());
        result.extend_from_slice(&(len as u16).to_be_bytes());
        result.extend_from_slice(msg);

        (result, len as u16)
    }

    fn decode_header(header: [u8; 5]) -> (MessageType, u16, u16) {
        let message_type = MessageType::from(header[0]);
        let message_number = u16::from_be_bytes([header[1], header[2]]);
        let message_size = u16::from_be_bytes([header[3], header[4]]);
        (message_type, message_number, message_size)
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    const MAX_LEN: usize = 508u32 as usize;

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

        for i in 0..1000u32 {
            c1.writer_ref().write(&i.to_be_bytes()).unwrap();
        }


        sleep(Duration::from_secs(1));

        for i in 0..1000u32 {
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
