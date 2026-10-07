use chrono::{NaiveDate, Utc};
use std::collections::HashMap;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    AddressDaily,
    IpHourly,
}

struct AddressDayUsage {
    date: NaiveDate,
    count: u32,
}

struct IpHourUsage {
    window_start: i64,
    count: u32,
}

fn utc_hour_start(ts: i64) -> i64 {
    ts.div_euclid(3600) * 3600
}

pub struct InMemoryRateLimiter {
    address_usage: Mutex<HashMap<String, AddressDayUsage>>,
    ip_usage: Mutex<HashMap<String, IpHourUsage>>,
    address_daily_drips: u32,
    ip_hourly_requests: u32,
}

impl InMemoryRateLimiter {
    pub fn new(address_daily_drips: u32, ip_hourly_requests: u32) -> Self {
        Self {
            address_usage: Mutex::new(HashMap::new()),
            ip_usage: Mutex::new(HashMap::new()),
            address_daily_drips,
            ip_hourly_requests,
        }
    }

    /// Consume one drip for `address` and one request for `ip`.
    /// Rolls neither counter back if either cap would be exceeded.
    pub async fn check_and_consume(&self, address: &str, ip: &str) -> Result<(), LimitKind> {
        let now = Utc::now();
        let today = now.date_naive();
        let hour_start = utc_hour_start(now.timestamp());

        let mut addresses = self.address_usage.lock().await;
        let mut ips = self.ip_usage.lock().await;

        let address_usage = addresses
            .entry(address.to_string())
            .or_insert(AddressDayUsage {
                date: today,
                count: 0,
            });
        if address_usage.date != today {
            address_usage.date = today;
            address_usage.count = 0;
        }
        if address_usage.count >= self.address_daily_drips {
            return Err(LimitKind::AddressDaily);
        }

        let ip_usage = ips.entry(ip.to_string()).or_insert(IpHourUsage {
            window_start: hour_start,
            count: 0,
        });
        if ip_usage.window_start != hour_start {
            ip_usage.window_start = hour_start;
            ip_usage.count = 0;
        }
        if ip_usage.count >= self.ip_hourly_requests {
            return Err(LimitKind::IpHourly);
        }

        address_usage.count = address_usage.count.saturating_add(1);
        ip_usage.count = ip_usage.count.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn one_drip_allowed_second_same_day_blocked() {
        let limiter = InMemoryRateLimiter::new(1, 10);
        limiter
            .check_and_consume("0xabc", "1.1.1.1")
            .await
            .expect("first drip");
        assert_eq!(
            limiter
                .check_and_consume("0xabc", "1.1.1.1")
                .await
                .expect_err("second drip same day"),
            LimitKind::AddressDaily
        );
    }

    #[tokio::test]
    async fn different_address_still_allowed() {
        let limiter = InMemoryRateLimiter::new(1, 10);
        limiter
            .check_and_consume("0xabc", "1.1.1.1")
            .await
            .expect("first address");
        limiter
            .check_and_consume("0xdef", "1.1.1.1")
            .await
            .expect("different address");
    }

    #[tokio::test]
    async fn ip_hourly_cap() {
        let limiter = InMemoryRateLimiter::new(10, 2);
        limiter
            .check_and_consume("0xa", "9.9.9.9")
            .await
            .expect("1");
        limiter
            .check_and_consume("0xb", "9.9.9.9")
            .await
            .expect("2");
        assert_eq!(
            limiter
                .check_and_consume("0xc", "9.9.9.9")
                .await
                .expect_err("third from same IP"),
            LimitKind::IpHourly
        );
        limiter
            .check_and_consume("0xc", "8.8.8.8")
            .await
            .expect("different IP");
    }
}
