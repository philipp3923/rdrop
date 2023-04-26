use std::fmt::{Debug, Display, Error, Formatter};
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use reqwest::get;
use socket2::Domain;
use crate::{IPV4_URL, IPV6_URL};

pub trait Address : Sized + Display + Debug{
    fn get_string(&self) -> String;
    fn from_str(address: &str) -> Result<Self, String> where Self: Sized;
    fn from_local() -> Self where Self: Sized;
    fn from_standard() -> Self where Self: Sized;
    fn from_public() -> Result<Self, String> where Self: Sized;
    fn to_socket_addr(self) -> IpAddr;
    fn get_domain() -> Domain;
}

#[derive(Debug)]
pub struct Ipv4([u8; 4]);

#[derive(Debug)]
pub struct Ipv6([u16; 8]);

impl Ipv4 {
    pub fn new(address: [u8; 4]) -> Ipv4 {
        return Ipv4(address);
    }
    pub fn to_parts(self) -> [u8; 4] {
        return self.0;
    }
}

impl Ipv6 {
    pub fn new(address: [u16; 8]) -> Ipv6 {
        return Ipv6(address);
    }
    pub fn to_parts(self) -> [u16; 8] {
        return self.0;
    }
}

impl Address for Ipv4 {

    fn get_string(&self) -> String {
        return format!("{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3]);
    }

    fn from_str(address: &str) -> Result<Ipv4, String> {
        let parts_str = address.replace("\n", "");
        let mut parts_str = parts_str.split(".").into_iter();
        let mut parts: [u8; 4] = [0; 4];

        for i in 0..4 as usize {
            let next_str = match parts_str.next() {
                None => return Err(format!("address is too small {}/{}", i, 8)),
                Some(num) => num
            };

            parts[i] = match u8::from_str_radix(next_str, 10) {
                Ok(num) => num,
                Err(_) => return Err(format!("parse failed at position {}. '{}' is not decimal.", i, next_str))
            };
        }

        Ok(Ipv4::new(parts))
    }

    fn from_local() -> Ipv4 {
        return Ipv4::new([127,0,0,1]);
    }

    fn from_standard() -> Ipv4 {
        return Ipv4::new([0,0,0,0]);
    }

    fn from_public() -> Result<Ipv4, String> where Self: Sized {
        let response = match reqwest::blocking::get(IPV4_URL) {
            Ok(result) => result,
            Err(_) => return Err(format!("Http Get Request failed")),
        };

        let response = match response.text() {
            Ok(response) => response,
            Err(_) => return Err(format!("Reading response body failed")),
        };

        return Ipv4::from_str(&response);
    }

    fn to_socket_addr(self) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(self.to_parts()))
    }

    fn get_domain() -> Domain {
        return Domain::IPV4;
    }
}

impl Address for Ipv6 {

    fn get_string(&self) -> String {
        return format!("{:X}:{:X}:{:X}:{:X}:{:X}:{:X}:{:X}:{:X}", self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7]);
    }

    fn from_str(address: &str) -> Result<Ipv6, String> {
        let parts_str = address.replace("\n", "");
        let mut parts_str = parts_str.split(":").into_iter();
        let mut parts: [u16; 8] = [0; 8];

        for i in 0..8 as usize {
            let next_str = match parts_str.next() {
                None => return Err(format!("address is too small {}/{}", i, 8)),
                Some(num) => num
            };

            parts[i] = match u16::from_str_radix(next_str, 16) {
                Ok(num) => num,
                Err(_) => return Err(format!("parse failed at position {}. '{}' is not hexadecimal.", i, next_str))
            };
        }

        Ok(Ipv6::new(parts))
    }

    fn from_local() -> Ipv6 {
        return Ipv6::new([0,0,0,0,0,0,0,1]);
    }

    fn from_standard() -> Ipv6  {
        return Ipv6::new([0,0,0,0,0,0,0,0]);

    }

    fn from_public() -> Result<Ipv6, String> {
        let response = match reqwest::blocking::get(IPV6_URL) {
            Ok(result) => result,
            Err(_) => return Err(format!("Http Get Request failed")),
        };

        let response = match response.text() {
            Ok(response) => response,
            Err(_) => return Err(format!("Reading response body failed")),
        };

        return Ipv6::from_str(&response);
    }

    fn to_socket_addr(self) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(self.to_parts()))
    }

    fn get_domain() -> Domain {
        return Domain::IPV6;
    }
}

impl Display for Ipv4 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0],self.0[1],self.0[2],self.0[3])
    }
}

impl Display for Ipv6 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}:{:X}:{:X}", self.0[0],self.0[1],self.0[2],self.0[3],self.0[4],self.0[5],self.0[6],self.0[7])
    }
}