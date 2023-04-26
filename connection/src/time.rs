use crate::SNTP_SERVER;
use chrono::{Timelike, Utc};
use rsntp::SntpClient;
use std::time::Duration;

pub(crate) struct Synchronizer {
    delta: Duration,
}

impl Synchronizer {
    pub fn new() -> Result<Synchronizer, String> {
        let client = SntpClient::new();
        let result = match client.synchronize(SNTP_SERVER) {
            Ok(value) => value,
            Err(_) => return Err(format!("failed to synchronize with sntp server")),
        };

        let delta: Duration = result.clock_offset().abs_as_std_duration().unwrap();

        return Ok(Synchronizer { delta });
    }

    pub fn wait_time(&self) -> Duration {
        let now = Utc::now();
        let target = now.with_nanosecond(0).unwrap() + chrono::Duration::seconds(1);
        let diff = target - now;
        return diff.to_std().unwrap() + self.delta;
    }
}
