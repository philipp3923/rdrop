use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::{JoinHandle};
use std::time::{Duration, Instant};

const MSG_RESEND_DELAY: Duration = Duration::from_millis(100);
const CONNECT_MSG_INTERVAL: Duration = Duration::from_millis(50);

pub struct WaitingConnection {
    udp_socket: UdpSocket,
}

impl WaitingConnection {
    pub fn new(port: Option<u16>) -> Result<WaitingConnection, ()> {
        let bind_addr = IpAddr::from(Ipv6Addr::from(0));
        let bind_addr = SocketAddr::new(bind_addr, port.unwrap_or(0));
        let udp_socket = match UdpSocket::bind(&bind_addr) {
            Ok(socket) => socket,
            Err(_) => return Err(()),
        };

        Ok(WaitingConnection { udp_socket })
    }

    pub fn connect(
        self,
        peer: Ipv6Addr,
        port: u16,
        timeout: Option<Duration>,
    ) -> Result<ActiveConnection, ()> {
        let peer_addr = IpAddr::from(peer);
        let peer_addr = SocketAddr::new(peer_addr, port);

        if self.udp_socket.connect(&peer_addr).is_err() {
            return Err(());
        }

        let mut active_connection = ActiveConnection::new(self.udp_socket).unwrap();

        return match active_connection.ping(timeout) {
            Ok(_) => Ok(active_connection),
            Err(_) => Err(()),
        };
    }

    pub fn get_port(&self) -> u16 {
        self.udp_socket.local_addr().unwrap().port()
    }
}

pub struct ActiveConnection {
    udp_socket: UdpSocket,
    send_counter: u8,
    message_receiver: Receiver<Vec<u8>>,
    acknowledgement_receiver: Receiver<u8>,
    receiver_handle: Option<JoinHandle<()>>,
    stop_thread: Sender<()>,
}

impl Drop for ActiveConnection {
    fn drop(&mut self) {
        self.stop_thread.send(()).unwrap();
        self.receiver_handle.take().unwrap().join().unwrap_or(());
    }
}

impl ActiveConnection {
    fn new(udp_socket: UdpSocket) -> Result<ActiveConnection, ()> {
        let receiver_socket = udp_socket.try_clone().unwrap();
        let (message_sender, message_receiver) = mpsc::channel::<Vec<u8>>();
        let (acknowledgement_sender, acknowledgement_receiver) = mpsc::channel::<u8>();
        let (stop_sender, stop_receiver) = mpsc::channel::<()>();

        if receiver_socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .is_err()
        {
            return Err(());
        }

        let receiver_handle = thread::spawn(move || {
            let mut msg_counter = 0;

            loop {
                if stop_receiver.try_recv().is_ok() {
                    println!("shutting down");
                    return;
                }

                let mut header = [0u8; 6];

                match receiver_socket.peek(header.as_mut_slice()) {
                    Ok(_) => {}
                    Err(_) => continue,
                }

                let msg_type = header[0];
                let msg_number = header[1];

                match msg_type {
                    0xCC => {
                        receiver_socket.recv(header.as_mut_slice()).unwrap();

                        println!("CC {}", msg_number);

                        if msg_number == msg_counter {
                            msg_counter = match msg_counter {
                                255 => 0,
                                x => x + 1,
                            };
                        }

                        if receiver_socket.send([0xAA, msg_number].as_slice()).is_err() {
                            println!("send err");
                            return;
                        }
                    }
                    0xAA => {
                        receiver_socket.recv(header.as_mut_slice()).unwrap();

                        println!("AA {}", msg_number);

                        if acknowledgement_sender.send(msg_number).is_err() {
                            return;
                        }
                    }
                    0xDD => {
                        let msg_len = u32::from_be_bytes(header[2..6].try_into().unwrap());

                        let mut msg_content = vec![0u8; msg_len as usize + 6];
                        for _ in 0..msg_len {
                            msg_content.push(0);
                        }

                        let _actual_len = match receiver_socket.recv(msg_content.as_mut_slice()) {
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

                            if message_sender.send(msg_content).is_err() {
                                return;
                            }
                        }

                        if receiver_socket.send([0xAA, msg_number].as_slice()).is_err() {
                            return;
                        }
                    }
                    _ => continue,
                }
            }
        });

        return Ok(ActiveConnection {
            udp_socket,
            send_counter: 0,
            acknowledgement_receiver,
            message_receiver,
            receiver_handle: Some(receiver_handle),
            stop_thread: stop_sender,
        });
    }

    pub fn ping(&mut self, timeout: Option<Duration>) -> Result<(), ()> {
        let now = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(0));

        while timeout.is_zero() || now.elapsed() <= timeout {
            if self.udp_socket.send(&[0xCC, self.send_counter]).is_err() {
                println!("send err");
                return Err(());
            }

            match self.acknowledgement_receiver.recv_timeout(MSG_RESEND_DELAY) {
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
        return Err(());
    }

    pub fn send(&mut self, msg: &[u8], timeout: Option<Duration>) -> Result<(), ()> {
        if msg.len() > 2 ^ 32 {
            return Err(());
        }

        let now = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(0));
        let msg = self.prepare_msg(msg);

        while timeout.is_zero() || now.elapsed() <= timeout {
            if self.udp_socket.send(msg.as_slice()).is_err() {
                return Err(());
            }

            match self.acknowledgement_receiver.recv_timeout(MSG_RESEND_DELAY) {
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
        return Err(());
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

    pub fn try_read(&mut self) -> Result<Vec<u8>, ()> {
        return match self.message_receiver.try_recv() {
            Ok(msg) => Ok(msg),
            Err(_) => Err(()),
        };
    }

    pub fn read(&mut self, timeout: Option<Duration>) -> Result<Vec<u8>, ()> {
        return match timeout {
            None => match self.message_receiver.recv() {
                Ok(msg) => Ok(msg),
                Err(_) => Err(()),
            },
            Some(t) => match self.message_receiver.recv_timeout(t) {
                Ok(msg) => Ok(msg),
                Err(_) => Err(()),
            },
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_port() {
        let w1 = WaitingConnection::new(None).unwrap();
        assert!(WaitingConnection::new(Some(w1.get_port())).is_err());
    }

    #[test]
    fn test_prepare_local() {
        for _ in 0..2 {
            println!("new run");
            sleep(Duration::from_secs(1));
            let (c1, c2) = prepare_local();
            drop(c1);
            drop(c2);
        }
    }

    fn prepare_local() -> (ActiveConnection, ActiveConnection) {
        let ipv6 = Ipv6Addr::from(0);
        let timeout = Duration::from_secs(2);
        let w1 = WaitingConnection::new(None).unwrap();
        let w2 = WaitingConnection::new(None).unwrap();

        let p1 = w1.get_port();
        let p2 = w2.get_port();

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
        let ipv6 = Ipv6Addr::from(0);
        let timeout = Duration::from_millis(5);
        let w1 = WaitingConnection::new(None).unwrap();
        let w2 = WaitingConnection::new(None).unwrap();

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
        let ipv6 = Ipv6Addr::from(0);
        let timeout = Duration::from_millis(10000);
        let w1 = WaitingConnection::new(None).unwrap();
        let w2 = WaitingConnection::new(None).unwrap();

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

        assert!(c1.send([1, 2, 3, 4].as_slice(), Some(timeout)).is_ok());
        assert!(c2.send([1, 2, 3, 4].as_slice(), Some(timeout)).is_ok());

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_read_local() {
        let (mut c1, mut c2) = prepare_local();
        let msg = [1, 2, 3, 4];
        let timeout = Duration::from_secs(2);

        c1.send(msg.as_slice(), Some(timeout)).unwrap();
        c2.send(msg.as_slice(), Some(timeout)).unwrap();

        assert_eq!(c1.read(Some(timeout)).unwrap(), msg);
        assert_eq!(c2.read(Some(timeout)).unwrap(), msg);

        drop(c1);
        drop(c2);
    }

    #[test]
    fn test_stress_local() {
        let (mut c1, mut c2) = prepare_local();
        let timeout = Duration::from_secs(2);

        for i in 0..10000u32 {
            c1.send(&i.to_be_bytes(), Some(timeout)).unwrap();
        }

        for i in 0..10000u32 {
            assert_eq!(
                u32::from_be_bytes(c2.try_read().unwrap().as_slice().try_into().unwrap()),
                i
            );
        }

        drop(c1);
        drop(c2);
    }
}
