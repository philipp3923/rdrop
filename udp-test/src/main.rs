use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::str::FromStr;
use std::time::Duration;
use connection::time::Synchronizer;

fn main() {
    let socket = UdpSocket::bind("0.0.0.0:2001").expect("binding socket failed");
    let address = IpAddr::from_str("91.56.29.75").expect("failed to parse ip");
    let socket_address = SocketAddr::new(address, 2000);
    let mut synchro = Synchronizer::new(false).expect("failed to create synchronizer");
    let mut count: u8 = 0;

    socket.set_read_timeout(Some(Duration::from_millis(100))).expect("failed to change timeout");
    socket.set_nonblocking(false).expect("failed to set blocking");

    loop {
        std::thread::sleep(synchro.wait_time());

        socket.send_to([count].as_slice(), socket_address).expect("failed to send");
        count+=1;
        let mut buf = [0u8; 1];

        match socket.recv_from(buf.as_mut_slice()) {
            Ok(_) => println!("Received: {}",buf[0]),
            Err(_) => continue,
        }
    }
}
