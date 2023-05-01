use std::fmt::format;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::thread::sleep;
use std::time;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use chrono::{Local, Utc};
use rand::{Rng, thread_rng};
use socket2::{Domain, SockAddr, Socket, Type};
use crate::protocol::Role::Server;

pub fn connect(udp_socket: &mut UdpSocket) -> Result<(), String> {
    if udp_socket.set_read_timeout(Some(Duration::from_millis(50))).is_err() {
        return Err(format!("changing read timeout failed"));
    }


    let mut buf = [0u8; 1];

    while buf[0] != 0xFF {
        if udp_socket.send(&[0xFF]).is_err() {
            return Err(format!("sending failed"));
        }

        //println!("WAITING");

        match udp_socket.recv(buf.as_mut_slice()) {
            _ => continue,
        }

    }

    println!("CONNECTED");

    if udp_socket.send(&[0xAA]).is_err() {
        return Err(format!("sending failed"));
    }

    while buf[0] == 0xFF {
        match udp_socket.recv(buf.as_mut_slice()) {
            _ => continue,
        }
    }

    if udp_socket.set_read_timeout(None).is_err() {
        return Err(format!("changing read timeout failed"));
    }

    return Ok(());
}

pub fn handshake(mut udp_socket: UdpSocket) -> Result<TcpStream, String> {
    if udp_socket.peer_addr().is_err() {
        return Err(format!("socket is not connected"));
    }

    let role = match negotiate_roles(&mut udp_socket) {
        Ok(role) => role,
        Err(_) => return Err(format!("negotiating roles failed")),
    };
    //let role = Server;

    let bind_addr = IpAddr::from(Ipv6Addr::from(0));
    let local_addr = SocketAddr::new(bind_addr, 0);
    let tcp_socket =  match Socket::new(Domain::IPV6, Type::STREAM, None) {
        Ok(socket) => socket,
        Err(_) => return Err(format!("creating tcp socket failed")),
    };

    if tcp_socket.bind(&SockAddr::from(local_addr)).is_err() {
        return Err(format!("binding tcp socket failed"));
    }

    let partner_port = match exchange_ports(&mut udp_socket, tcp_socket.local_addr().unwrap().as_socket().unwrap().port()) {
        Ok(p) => p,
        Err(_) => return Err(format!("exchanging ports failed")),
    };

    println!("my port: {}\npartner port: {}", tcp_socket.local_addr().unwrap().as_socket().unwrap().port(), partner_port);
    return match role {
        Role::Client => {
            println!("Client");
            if sync_client(& mut udp_socket).is_err() {
                return Err(format!("syncing failed"));
            }

            Ok(upgrade_client(udp_socket, tcp_socket, partner_port).unwrap())
        }
        Role::Server => {
            println!("Server");
            let (diff, delay) = match sync_server(& mut udp_socket, 100) {
                Ok(d) => d,
                Err(_) => return Err(format!("syncing failed")),
            };

            Ok(upgrade_server(udp_socket, tcp_socket, partner_port, diff, delay).unwrap())
        }
    };
}

enum Role{
    Client,
    Server
}

fn upgrade_server(udp_socket: UdpSocket, tcp_socket: Socket, port: u16, diff: i128, delay: u128) -> Result<TcpStream, String> {
    println!("now:   {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());
    println!("diff:  {}", diff);
    println!("delay: {}", delay);

    let mut buf = [0; 1];
    let mut partner_address = udp_socket.peer_addr().unwrap();
    partner_address.set_port(port);
    let partner_address = SockAddr::from(partner_address);

    if udp_socket.recv(buf.as_mut_slice()).is_err() {
        return Err(format!("receiving failed"));
    }

    let connect_time = (SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() + delay*10) as i128 - diff + 1000000 * 60;

    if udp_socket.send(&connect_time.to_be_bytes()).is_err() {
        return Err(format!("sending failed"));
    }

    sleep(Duration::from_nanos((delay*10 + 1000000 * 60) as u64));

    println!("Local: {}", Local::now().timestamp_nanos());

    return match tcp_socket.connect(&partner_address) {
        Ok(_) => Ok(TcpStream::from(tcp_socket)),
        Err(_e) => { println!("{}", _e); Err(format!("connecting tcp socket failed"))},
    }
}

fn upgrade_client(udp_socket: UdpSocket, tcp_socket: Socket, port: u16) -> Result<TcpStream, String> {
    let mut buf = [0; 16];
    let mut partner_address = udp_socket.peer_addr().unwrap();
    partner_address.set_port(port);
    let partner_address = SockAddr::from(partner_address);

    if udp_socket.send(&[0xAB]).is_err() {
        return Err(format!("sending failed"));
    }

    if udp_socket.recv(buf.as_mut_slice()).is_err() {
        return Err(format!("receiving failed"));
    }

    let sleep_duration = u128::from_be_bytes(buf) - SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    sleep(Duration::from_nanos(sleep_duration as u64));

    println!("Local: {}", Local::now().timestamp_nanos());

    return match tcp_socket.connect(&partner_address) {
        Ok(_) => Ok(TcpStream::from(tcp_socket)),
        Err(_e) => { println!("{}", _e); Err(format!("connecting tcp socket failed"))},
    }

}

fn exchange_ports(udp_socket: &mut UdpSocket, port: u16) -> Result<u16, String>{
    if udp_socket.send(&port.to_be_bytes()).is_err() {
        return Err(format!("sending failed"));
    }

    let mut buf = [0u8; 2];

    if udp_socket.recv(buf.as_mut_slice()).is_err() {
        return Err(format!("receiving failed"));
    }

    return Ok(u16::from_be_bytes(buf));
}

fn negotiate_roles(udp_socket: &mut UdpSocket) -> Result<Role, String> {
    let mut rng = thread_rng();
    let mut my_number = [rng.gen(), rng.gen()];
    let mut buf = [0u8; 2];

    loop {
        if udp_socket.send(&my_number).is_err() {
            return Err(format!("sending failed"));
        }

        if udp_socket.recv(buf.as_mut_slice()).is_err() {
            return Err(format!("receiving failed"));
        }

        if buf != my_number {
            break;
        }
    }

    return if my_number > buf {
        Ok(Role::Server)
    } else {
        Ok(Role::Client)
    }
}

fn sync_server(udp_socket: &mut UdpSocket, mut samples: u8) -> Result<(i128, u128), String> {
    if samples == 0 {
        return Err(format!("samples cannot be 0"));
    }

    if udp_socket.set_read_timeout(None).is_err() {
        return Err(format!("changing read timeout failed"));
    }
    let mut buf = [0; 1];

    if udp_socket.send(&[0xAB]).is_err() {
        return Err(format!("sending failed"));
    }

    if udp_socket.recv(buf.as_mut_slice()).is_err() {
        return Err(format!("receiving failed"));
    }

    if buf[0] != 0xAB {
        return Err(format!("illegal ready response"));
    }

    let mut max: u128 = 0;
    let mut diffs = Vec::new();
    for _ in 0..samples {
        let mut buf = [0u8; 16];
        let start = SystemTime::now();

        if udp_socket.send(&[0xBB]).is_err() {
            return Err(format!("sending failed"));
        }

        if udp_socket.recv(buf.as_mut_slice()).is_err() {
            return Err(format!("receiving failed"));
        }

        let now = SystemTime::now();
        let now_nanos = now.duration_since(UNIX_EPOCH).unwrap().as_nanos();

        let elapsed = start.elapsed().unwrap().as_nanos();

        let partner_now_nanos = u128::from_be_bytes(buf);
        let diff = now_nanos as i128 - partner_now_nanos as i128 - elapsed as i128 / 2; // Zeitdifferenz symmetrischer Jitter
        diffs.push(diff);
        if elapsed > max {
            max = elapsed;
        }

    }

    if udp_socket.send(&[0xCC]).is_err() {
        return Err(format!("sending failed"));
    }

    diffs.sort();

    println!("{:#?}", diffs.as_slice());

    let median_diff = if diffs.len() % 2 == 0 {
        (diffs[diffs.len() / 2] + diffs[diffs.len() / 2 - 1]) / 2
    } else {
        diffs[diffs.len() / 2]
    };
    println!("med: {}", median_diff);
    return Ok((median_diff, max));
}

fn sync_client(udp_socket: &mut UdpSocket) -> Result<(), String> {
    if udp_socket.set_read_timeout(None).is_err() {
        return Err(format!("changing read timeout failed"));
    }

    let mut buf = [0; 1];

    if udp_socket.recv(buf.as_mut_slice()).is_err() {
        return Err(format!("receiving failed"));
    }

    if buf[0] != 0xAB {
        return Err(format!("illegal ready request"));
    }

    if udp_socket.send(&[0xAB]).is_err() {
        return Err(format!("sending failed"));
    }

    while buf[0] != 0xCC {
        if udp_socket.recv(buf.as_mut_slice()).is_err() {
            return Err(format!("receiving failed"));
        }

        let now = SystemTime::now();
        let now_nanos = now.duration_since(UNIX_EPOCH).unwrap().as_nanos();
        if udp_socket.send(&now_nanos.to_be_bytes()).is_err() {
            return Err(format!("sending failed"));
        }
    }

    return Ok(());
}