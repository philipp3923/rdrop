use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::Duration;

pub struct UdpConnection {
    udp_socket: UdpSocket,
    send_count: u32,
    receive_count: u32,
}

impl UdpConnection {
    pub fn new(port: Option<u16>) -> Result<UdpConnection, ()> {
        let bind_addr = IpAddr::from(Ipv6Addr::from(0));
        let bind_addr = SocketAddr::new(bind_addr, port.unwrap_or(0));
        return match UdpSocket::bind(&bind_addr) {
            Ok(socket) => Ok(UdpConnection {
                udp_socket: socket,
                send_count: 0,
                receive_count: 0,
            }),
            Err(_) => Err(()),
        };
    }

    pub fn connect(&mut self, peer: Ipv6Addr, port: u16) -> Result<(), ()> {
        let peer_addr = IpAddr::from(peer);
        let peer_addr = SocketAddr::new(peer_addr, port);

        if self.udp_socket.connect(&peer_addr).is_err() {
            return Err(());
        }

        return Ok(());
    }

    pub fn send_and_receive(&mut self, msg: &[u8]) -> Result<Vec<u8>, ()> {
        if self
            .udp_socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .is_err()
        {
            return Err(());
        }

        let msg = self.prepare_msg(msg);

        'iter: for _ in 0..10 {
            if self.udp_socket.send(msg.as_slice()).is_err() {
                return Err(());
            }

            match self.parse_msg() {
                Ok((mut num, mut msg)) => {
                    if num == 0 {
                        return Err(());
                    }

                    while num < self.receive_count {
                        (num, msg) = match self.parse_msg() {
                            Ok(res) => res,
                            Err(_) => continue 'iter,
                        }
                    }

                    if num > self.receive_count + 1 {
                        return Err(());
                    }

                    return Ok(msg);
                }
                Err(_) => continue,
            }
        }

        return Err(());
    }

    fn prepare_msg(&mut self, msg: &[u8]) -> Vec<u8> {
        let len = msg.len();
        let mut result = Vec::with_capacity(len + 4 + 4);

        result.extend_from_slice(&self.send_count.to_be_bytes());
        result.extend_from_slice(&(len as u32).to_be_bytes());
        result.extend_from_slice(msg);

        self.send_count += 1;

        result
    }

    fn parse_msg(&mut self) -> Result<(u32, Vec<u8>), ()> {
        let mut msg = [0u8; 8];
        let len = match self.udp_socket.recv(msg.as_mut_slice()) {
            Ok(l) => l as u32,
            Err(_) => return Err(()),
        };

        // message is too short
        if len < 8 {
            return Err(());
        }

        let msg_number = u32::from_be_bytes(msg[0..4].try_into().unwrap());
        let msg_len = u32::from_be_bytes(msg[4..8].try_into().unwrap());

        let mut msg = vec![0u8; msg_len as usize];

        let len = match self.udp_socket.recv(msg.as_mut_slice()) {
            Ok(l) => l as u32,
            Err(_) => return Err(()),
        };

        // read message is shorter than given length
        if msg_len < len {
            return Err(());
        }

        Ok((msg_number, msg))
    }
}
