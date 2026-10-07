use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    AddressDaily,
    IpHourly,
}

#[derive(Debug)]
pub enum StoreError {
    Limit(LimitKind),
    Database(rusqlite::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit(LimitKind::AddressDaily) => {
                write!(f, "address daily drip limit")
            }
            Self::Limit(LimitKind::IpHourly) => write!(f, "ip hourly request limit"),
            Self::Database(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Database(err)
    }
}

#[derive(Debug, Clone)]
pub struct Reservation {
    pub address: String,
    pub ip: String,
}

fn utc_hour_start(ts: i64) -> i64 {
    ts.div_euclid(3600) * 3600
}

fn utc_day(now: DateTime<Utc>) -> String {
    now.date_naive().format("%Y-%m-%d").to_string()
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS address_drip (
  address     TEXT PRIMARY KEY,
  day         TEXT NOT NULL,
  count       INTEGER NOT NULL,
  last_tx     TEXT,
  updated_at  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS ip_window (
  ip           TEXT NOT NULL,
  window_start INTEGER NOT NULL,
  count        INTEGER NOT NULL,
  PRIMARY KEY (ip, window_start)
);
";

pub struct FaucetStore {
    conn: Mutex<Connection>,
    address_daily_drips: u32,
    ip_hourly_requests: u32,
}

impl FaucetStore {
    pub fn open(
        path: impl AsRef<Path>,
        address_daily_drips: u32,
        ip_hourly_requests: u32,
    ) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(StoreError::Io)?;
            }
        }
        let conn = Connection::open(path)?;
        Self::init(conn, address_daily_drips, ip_hourly_requests)
    }

    fn init(
        conn: Connection,
        address_daily_drips: u32,
        ip_hourly_requests: u32,
    ) -> Result<Self, StoreError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
            address_daily_drips,
            ip_hourly_requests,
        })
    }

    pub fn reserve(
        &self,
        address: &str,
        ip: &str,
        now: DateTime<Utc>,
    ) -> Result<Reservation, StoreError> {
        let today = utc_day(now);
        let hour_start = utc_hour_start(now.timestamp());
        let now_ts = now.timestamp();

        let conn = self.conn.lock().expect("faucet store mutex");
        let tx = conn.unchecked_transaction()?;

        let addr_count: u32 = tx
            .query_row(
                "SELECT count, day FROM address_drip WHERE address = ?1",
                [address],
                |row| {
                    let count: u32 = row.get(0)?;
                    let day: String = row.get(1)?;
                    Ok(if day == today { count } else { 0 })
                },
            )
            .optional()?
            .unwrap_or(0);

        if addr_count >= self.address_daily_drips {
            return Err(StoreError::Limit(LimitKind::AddressDaily));
        }

        let ip_count: u32 = tx
            .query_row(
                "SELECT count FROM ip_window WHERE ip = ?1 AND window_start = ?2",
                params![ip, hour_start],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);

        if ip_count >= self.ip_hourly_requests {
            return Err(StoreError::Limit(LimitKind::IpHourly));
        }

        let next_addr = addr_count.saturating_add(1);
        tx.execute(
            "INSERT INTO address_drip (address, day, count, last_tx, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4)
             ON CONFLICT(address) DO UPDATE SET
               day = excluded.day,
               count = excluded.count,
               updated_at = excluded.updated_at",
            params![address, today, next_addr, now_ts],
        )?;
        tx.execute(
            "INSERT INTO ip_window (ip, window_start, count)
             VALUES (?1, ?2, 1)
             ON CONFLICT(ip, window_start) DO UPDATE SET count = count + 1",
            params![ip, hour_start],
        )?;
        tx.commit()?;

        Ok(Reservation {
            address: address.to_string(),
            ip: ip.to_string(),
        })
    }

    pub fn commit(&self, address: &str, tx_hash: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("faucet store mutex");
        conn.execute(
            "UPDATE address_drip SET last_tx = ?1 WHERE address = ?2",
            params![tx_hash, address],
        )?;
        Ok(())
    }

    pub fn release(&self, address: &str, ip: &str, now: DateTime<Utc>) -> Result<(), StoreError> {
        let today = utc_day(now);
        let hour_start = utc_hour_start(now.timestamp());
        let now_ts = now.timestamp();
        let conn = self.conn.lock().expect("faucet store mutex");
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE address_drip
             SET count = MAX(0, count - 1), updated_at = ?1
             WHERE address = ?2 AND day = ?3",
            params![now_ts, address, today],
        )?;
        tx.execute(
            "UPDATE ip_window
             SET count = MAX(0, count - 1)
             WHERE ip = ?1 AND window_start = ?2",
            params![ip, hour_start],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn purge_older_than(&self, cutoff: DateTime<Utc>) -> Result<(), StoreError> {
        let cutoff_ts = cutoff.timestamp();
        let conn = self.conn.lock().expect("faucet store mutex");
        conn.execute(
            "DELETE FROM address_drip WHERE updated_at < ?1",
            [cutoff_ts],
        )?;
        conn.execute("DELETE FROM ip_window WHERE window_start < ?1", [cutoff_ts])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn memory_store(address_daily: u32, ip_hourly: u32) -> FaucetStore {
        let conn = Connection::open_in_memory().expect("memory sqlite");
        FaucetStore::init(conn, address_daily, ip_hourly).expect("init")
    }

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).expect("timestamp")
    }

    #[test]
    fn one_drip_allowed_second_same_day_blocked() {
        let store = memory_store(1, 10);
        let t = now();
        store.reserve("0xabc", "1.1.1.1", t).expect("first drip");
        match store.reserve("0xabc", "1.1.1.1", t) {
            Err(StoreError::Limit(LimitKind::AddressDaily)) => {}
            other => panic!("expected AddressDaily, got {other:?}"),
        }
    }

    #[test]
    fn different_address_still_allowed() {
        let store = memory_store(1, 10);
        let t = now();
        store.reserve("0xabc", "1.1.1.1", t).expect("first address");
        store
            .reserve("0xdef", "1.1.1.1", t)
            .expect("different address");
    }

    #[test]
    fn ip_hourly_cap() {
        let store = memory_store(10, 2);
        let t = now();
        store.reserve("0xa", "9.9.9.9", t).expect("1");
        store.reserve("0xb", "9.9.9.9", t).expect("2");
        match store.reserve("0xc", "9.9.9.9", t) {
            Err(StoreError::Limit(LimitKind::IpHourly)) => {}
            other => panic!("expected IpHourly, got {other:?}"),
        }
        store.reserve("0xc", "8.8.8.8", t).expect("different IP");
    }

    #[test]
    fn release_after_failure_allows_retry() {
        let store = memory_store(1, 1);
        let t = now();
        let reservation = store.reserve("0xabc", "1.1.1.1", t).expect("reserve");
        store
            .release(&reservation.address, &reservation.ip, t)
            .expect("release");
        store
            .reserve("0xabc", "1.1.1.1", t)
            .expect("retry after failed broadcast");
    }

    #[test]
    fn commit_sets_last_tx() {
        let store = memory_store(1, 1);
        let t = now();
        store.reserve("0xabc", "1.1.1.1", t).expect("reserve");
        store.commit("0xabc", "deadbeef").expect("commit");
        let conn = store.conn.lock().expect("mutex");
        let last_tx: String = conn
            .query_row(
                "SELECT last_tx FROM address_drip WHERE address = ?1",
                ["0xabc"],
                |row| row.get(0),
            )
            .expect("last_tx");
        assert_eq!(last_tx, "deadbeef");
    }

    #[test]
    fn open_creates_parent_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("faucet.db");
        let store = FaucetStore::open(&path, 1, 3).expect("open");
        let t = now();
        store.reserve("0xabc", "1.1.1.1", t).expect("reserve");
        assert!(path.exists());
    }

    #[test]
    fn purge_drops_rows_older_than_cutoff() {
        let store = memory_store(1, 1);
        let t = now();
        store.reserve("0xabc", "1.1.1.1", t).expect("reserve");
        store
            .purge_older_than(t + chrono::Duration::seconds(1))
            .expect("purge");
        store
            .reserve("0xabc", "1.1.1.1", t)
            .expect("row was purged");
    }
}
