use crate::SNTP_SERVER;
use chrono::{Timelike, Utc};
use rsntp::SntpClient;
use std::time::Duration;

pub struct Synchronizer {
    delta: Duration,
    signum: i8,
}

impl Synchronizer {
    pub fn new(with_delta: bool) -> Result<Synchronizer, String> {
        // #TODO
        if !with_delta {
            return Ok(Synchronizer {
                delta: Duration::new(0, 0),
                signum: 1,
            });
        }

        let client = SntpClient::new();
        let result = match client.synchronize(SNTP_SERVER) {
            Ok(value) => value,
            Err(_) => return Err(format!("failed to synchronize with sntp server")),
        };

        let signum = result.clock_offset().signum() as i8;
        let delta: Duration = match result.clock_offset().abs_as_std_duration() {
            Ok(d) => d,
            Err(_) => return Err(format!("failed to convert sntp server result")),
        };

        println!("delta:  {}", delta.as_secs_f32());
        println!("signum: {}", signum);

        return Ok(Synchronizer { delta, signum });
    }

    pub fn wait_time(&mut self) -> Duration {
        let mut now = Utc::now();

        if self.signum < 0 {
            now -= chrono::Duration::from_std(self.delta).unwrap();
        } else {
            now += chrono::Duration::from_std(self.delta).unwrap();
        }

        let target = now.with_nanosecond(0).unwrap() + chrono::Duration::seconds(1);

        let diff = (target - now).to_std().unwrap();

        println!("NOW:     {}", now);
        println!("TARGET:  {}", target);
        return diff;
    }
}
