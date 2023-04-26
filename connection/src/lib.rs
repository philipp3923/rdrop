use std::ops::Range;

pub mod client;
pub mod ip;
mod protocol;
mod time;

const PORT_RANGE: Range<u16> = 2000..3000;
const CONNECT_ATTEMPTS: u16 = 60;
const IPV6_URL: &str = "https://api64.ipify.org";
const IPV4_URL: &str = "https://api.ipify.org";
const SNTP_SERVER: &str = "3.de.pool.ntp.org";
