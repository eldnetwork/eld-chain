use chrono::{NaiveDate, Utc};
use std::collections::HashMap;
use tokio::sync::Mutex;

pub const DAILY_REQUEST_LIMIT: u64 = 10_000_000;

struct DailyUsage {
    date: NaiveDate,
    amount: u64,
}

pub struct InMemoryRateLimiter {
    request_usage: Mutex<HashMap<String, DailyUsage>>,
    daily_limit: u64,
}

impl InMemoryRateLimiter {
    pub fn new(daily_limit: u64) -> Self {
        Self {
            request_usage: Mutex::new(HashMap::new()),
            daily_limit,
        }
    }

    pub async fn check_and_consume(
        &self,
        requester: &str,
        requested_amount: u64,
    ) -> Result<(), u64> {
        let today = Utc::now().date_naive();
        let mut usage = self.request_usage.lock().await;

        let requester_usage = usage.entry(requester.to_string()).or_insert(DailyUsage {
            date: today,
            amount: 0,
        });

        if requester_usage.date != today {
            requester_usage.date = today;
            requester_usage.amount = 0;
        }

        let updated_amount = requester_usage.amount.saturating_add(requested_amount);
        if updated_amount > self.daily_limit {
            return Err(self.daily_limit.saturating_sub(requester_usage.amount));
        }

        requester_usage.amount = updated_amount;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn drip_exceeds_daily_limit() {
        let limiter = InMemoryRateLimiter::new(DAILY_REQUEST_LIMIT);
        let remaining = limiter
            .check_and_consume("0xabc", 1_000 * 1_000_000)
            .await
            .expect_err("drip is larger than the daily cap");
        assert_eq!(remaining, DAILY_REQUEST_LIMIT);
    }

    #[tokio::test]
    async fn consumes_until_the_daily_cap() {
        let limiter = InMemoryRateLimiter::new(100);
        limiter
            .check_and_consume("0xabc", 40)
            .await
            .expect("under the cap");
        let remaining = limiter
            .check_and_consume("0xabc", 70)
            .await
            .expect_err("over the cap");
        assert_eq!(remaining, 60);
        limiter
            .check_and_consume("0xabc", 60)
            .await
            .expect("exact remaining");
        limiter
            .check_and_consume("0xother", 100)
            .await
            .expect("other address has its own cap");
    }
}
