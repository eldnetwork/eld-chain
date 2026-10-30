//! Still-live pinboard posts, held in memory for frontier computation.
//!
//! Not part of `app_hash`. Rebuild from RocksDB after a restart.
//!
//! One ordered set is enough: inserts are rare, and the only query is the oldest
//! creation height among pins that have not expired yet (scanned when needed).

use std::collections::BTreeSet;

/// One still-live pin (expiry height, creation height, message id).
///
/// These values are fixed for the life of the entry. Leaving the set deletes the
/// pin; it does not rewrite the heights on that pin.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct LivePin {
    pub expires_height: i64,
    pub creation_height: i64,
    pub message_id: String,
}

/// Every pin that is still live.
///
/// Pins are ordered by expiry height (then creation height, then message id) so
/// finished pins can be dropped from the front. The oldest live creation height
/// is found by scanning the remaining pins when asked.
#[derive(Debug, Default)]
pub struct LivePinSet {
    /// Ordered by expiry height, then creation height, then message id.
    by_expiry: BTreeSet<LivePin>,
}

impl LivePinSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new live pin.
    ///
    /// Ignores pins that never live (expiry at or before creation).
    pub fn insert(&mut self, creation_height: i64, expires_height: i64, message_id: String) {
        if expires_height <= creation_height {
            return;
        }
        self.by_expiry.insert(LivePin {
            expires_height,
            creation_height,
            message_id,
        });
    }

    /// Drop pins that have expired by the given block height.
    ///
    /// Returns the pins removed, in expiry order. Pins that expire after this
    /// height stay in the set.
    pub fn expire_up_to(&mut self, height: i64) -> Vec<LivePin> {
        let mut dropped = Vec::new();
        while let Some(pin) = self.by_expiry.first() {
            if pin.expires_height > height {
                break;
            }
            if let Some(pin) = self.by_expiry.pop_first() {
                dropped.push(pin);
            }
        }
        dropped
    }

    /// Minimum creation height among pins that are still live.
    pub fn oldest_creation_height(&self) -> Option<i64> {
        self.by_expiry.iter().map(|p| p.creation_height).min()
    }
}

/// Build a set from stored pinboard pins.
///
/// Each item is `(creation_height, expires_height, message_id)`. Expired pins
/// stay in the set; EndBlock drops them. Pins that never live
/// (`expires_height <= creation_height`) are ignored.
impl From<Vec<(u64, u64, String)>> for LivePinSet {
    fn from(pins: Vec<(u64, u64, String)>) -> Self {
        let mut set = Self::new();
        for (creation_height, expires_height, message_id) in pins {
            let Ok(creation_height) = i64::try_from(creation_height) else {
                continue;
            };
            let Ok(expires_height) = i64::try_from(expires_height) else {
                continue;
            };
            set.insert(creation_height, expires_height, message_id);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_rejects_expires_at_or_before_creation() {
        let mut set = LivePinSet::new();
        set.insert(10, 10, "a".into());
        set.insert(10, 9, "b".into());
        assert_eq!(set.oldest_creation_height(), None);
    }

    #[test]
    fn oldest_is_minimum_creation_height() {
        let mut set = LivePinSet::new();
        set.insert(20, 120, "newer".into());
        set.insert(10, 110, "older".into());
        set.insert(15, 115, "mid".into());
        assert_eq!(set.oldest_creation_height(), Some(10));
    }

    fn dropped_ids(pins: &[LivePin]) -> Vec<&str> {
        pins.iter().map(|pin| pin.message_id.as_str()).collect()
    }

    #[test]
    fn expire_advances_oldest() {
        let mut set = LivePinSet::new();
        set.insert(10, 50, "a".into());
        set.insert(20, 60, "b".into());
        assert_eq!(dropped_ids(&set.expire_up_to(50)), vec!["a"]);
        assert_eq!(set.oldest_creation_height(), Some(20));
        assert_eq!(dropped_ids(&set.expire_up_to(60)), vec!["b"]);
        assert_eq!(set.oldest_creation_height(), None);
    }

    #[test]
    fn same_creation_height_distinct_message_ids() {
        let mut set = LivePinSet::new();
        set.insert(10, 100, "a".into());
        set.insert(10, 100, "b".into());
        assert_eq!(set.oldest_creation_height(), Some(10));
        assert_eq!(dropped_ids(&set.expire_up_to(100)), vec!["a", "b"]);
        assert_eq!(set.oldest_creation_height(), None);
    }

    #[test]
    fn duplicate_insert_is_idempotent() {
        let mut set = LivePinSet::new();
        set.insert(10, 100, "a".into());
        set.insert(10, 100, "a".into());
        assert_eq!(dropped_ids(&set.expire_up_to(100)), vec!["a"]);
        assert_eq!(set.oldest_creation_height(), None);
    }

    #[test]
    fn from_stored_pins_keeps_expired_and_skips_never_live() {
        let set = LivePinSet::from(vec![
            (10, 10, "dead".into()),
            (5, 40, "expired".into()),
            (20, 80, "live".into()),
        ]);
        assert_eq!(set.oldest_creation_height(), Some(5));
    }

    #[test]
    fn oldest_creation_is_not_confused_with_earliest_expiry() {
        // Pin A: created early, expires late. Pin B: created later, expires sooner.
        let mut set = LivePinSet::new();
        set.insert(10, 200, "a".into());
        set.insert(50, 100, "b".into());
        assert_eq!(set.oldest_creation_height(), Some(10));
        assert_eq!(dropped_ids(&set.expire_up_to(100)), vec!["b"]);
        assert_eq!(set.oldest_creation_height(), Some(10));
    }
}
