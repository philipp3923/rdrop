use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::str::FromStr;
use connection::time::Synchronizer;

fn main() {
    let socket = UdpSocket::bind("0.0.0.0:2000").expect("binding socket failed");
    let address = IpAddr::from_str("0.0.0.0").expect("failed to parse ip");
    let socket_address = SocketAddr::new(address, 2000);
    let mut synchro = Synchronizer::new(false).expect("failed to create synchronizer");
    let mut count: u8 = 0;
    loop {
        std::thread::sleep(synchro.wait_time());

        socket.send_to([count].as_slice(),socket_address);
        count+=1;
        let mut buf = [0u8; 1];

        socket.recv_from(buf.as_mut_slice()).expect("reading message failed");

        if buf[0] != 0 {
            println!("Received: {}",buf[0]);
        }

    }
}
