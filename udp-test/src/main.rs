use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::str::FromStr;
use std::time::Duration;
use connection::time::Synchronizer;

fn main() {
    let bind = IpAddr::from(Ipv6Addr::new(0,0,0,0,0,0,0,0));
    let socket = UdpSocket::bind(SocketAddr::new(bind, 2000)).expect("binding socket failed");
    let address = IpAddr::from_str("2003:F4:71A:B504:1014:BE5A:29F4:800D").expect("failed to parse ip");
    let socket_address = SocketAddr::new(address, 2000);
    let mut synchro = Synchronizer::new(false).expect("failed to create synchronizer");
    let mut count: u8 = 0;

    socket.set_read_timeout(Some(Duration::from_millis(10))).expect("failed to change timeout");
    socket.set_nonblocking(false).expect("failed to set blocking");

    loop {
        std::thread::sleep(synchro.wait_time());

        match socket.send_to([count].as_slice(), socket_address) {
            Ok(_) => {},
            Err(_) => continue,
        }

        count+=1;
        println!("Sent: {}",count);
        let mut buf = [0u8; 1];

        match socket.recv_from(buf.as_mut_slice()) {
            Ok(_) => println!("Received: {}",buf[0]),
            Err(_) => continue,
        }
    }
}
