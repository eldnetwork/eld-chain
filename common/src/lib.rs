#![doc = include_str!("../README.md")]
#![warn(missing_docs, unreachable_pub)]

// `missing_docs` is enforced on Address, Tx, Wallet, the typed IDs, and EldError.
// Other modules still have a large undocumented public surface.
#[allow(missing_docs)]
pub mod account;
pub mod address;
#[allow(missing_docs)]
pub mod cado;
#[allow(missing_docs)]
pub mod capacity;
pub mod capacity_challenge;
pub mod capacity_merkle_root;
#[allow(missing_docs)]
pub mod capacity_proof;
pub mod capacity_seed;
pub mod challenge_id;
pub mod coin;
#[allow(missing_docs)]
pub mod constants;
pub mod content_id;
pub mod error;
#[allow(missing_docs)]
pub mod fee;
mod hex_encoding;
#[allow(missing_docs)]
pub mod logging;
pub mod manifest_id;
#[allow(missing_docs)]
pub mod missing_content;
pub mod namespace;
pub mod nonce;
#[allow(missing_docs)]
pub mod pinboard;
#[allow(missing_docs)]
pub mod protocol_constants;
pub mod public_key;
#[allow(missing_docs)]
pub mod staking_account;
#[allow(missing_docs)]
pub mod storage;
#[allow(missing_docs)]
pub mod sync_msg;
pub mod tx;
#[allow(missing_docs)]
pub mod utils;
pub mod validation;
#[allow(missing_docs)]
pub mod validator;
pub mod wallet;

pub use address::Address;
pub use capacity_merkle_root::CapacityMerkleRoot;
pub use capacity_seed::CapacitySeed;
pub use challenge_id::ChallengeId;
pub use content_id::ContentId;
pub use manifest_id::ManifestId;
pub use public_key::PublicKey;
use tendermint::Time; // Re-export Address

/// Seconds since UNIX epoch
pub type Timespec = u64;

/// Converts a Tendermint [`Time`] to seconds since the Unix epoch.
///
/// # Errors
///
/// Returns [`error::EldError::ValidationError`] when `time` is before the Unix epoch.
///
/// # Examples
///
/// ```
/// use eld_common::to_timespec;
/// use tendermint::Time;
///
/// assert_eq!(to_timespec(Time::unix_epoch()).unwrap(), 0);
/// ```
pub fn to_timespec(time: Time) -> Result<Timespec, error::EldError> {
    time.duration_since(Time::unix_epoch())
        .map(|duration| duration.as_secs())
        .map_err(|e| error::EldError::ValidationError {
            field: "time".to_string(),
            value: time.to_rfc3339(),
            details: format!("Failed to calculate duration since Unix epoch: {e}"),
        })
}

pub use tx::create_event_attribute;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_timespec_unix_epoch_is_zero() {
        assert_eq!(to_timespec(Time::unix_epoch()).unwrap(), 0);
    }
}
