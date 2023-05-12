use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;
use sntpc::{NtpContext, NtpTimestampGenerator, NtpUdpSocket};

#[derive(Copy, Clone, Default)]
struct StdTimestampGen {
    duration: Duration,
}

impl NtpTimestampGenerator for StdTimestampGen {
    fn init(&mut self) {
        self.duration = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
    }

    fn timestamp_sec(&self) -> u64 {
        self.duration.as_secs()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        self.duration.subsec_micros()
    }
}

#[derive(Debug)]
struct UdpSocketWrapper(UdpSocket);

impl NtpUdpSocket for UdpSocketWrapper {
    fn send_to<T: ToSocketAddrs>(&self, buf: &[u8], addr: T) -> sntpc::Result<usize> {
        match self.0.send_to(buf, addr) {
            Ok(usize) => Ok(usize),
            Err(_) => Err(sntpc::Error::Network),
        }
    }

    fn recv_from(&self, buf: &mut [u8]) -> sntpc::Result<(usize, SocketAddr)> {
        match self.0.recv_from(buf) {
            Ok((size, addr)) => Ok((size, addr)),
            Err(_) => Err(sntpc::Error::Network),
        }
    }
}

pub fn get_diff() -> Result<(Duration, i64), crate::error::Error> {
    let ntp_context = NtpContext::new(StdTimestampGen::default());
    let socket = UdpSocketWrapper(UdpSocket::bind("0.0.0.0:0").expect("something"));
    //#TODO change time server to be dynamic
    let result = sntpc::get_time("83.168.200.199:123", socket, ntp_context);

    println!("{:?}", result);

    let result = result?;

    Ok((Duration::from_micros(if result.offset < 0 { result.offset * -1 } else { result.offset } as u64), result.offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_time() {
        assert!(get_diff().is_ok());
    }
}