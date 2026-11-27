//! Tendermint peer id (20 bytes), stored separately from the Tendermint type.
//!
//! Canonical text is 40 lowercase hex digits with no `0x` prefix, matching `node_info.id`
//! and the ids in the connected-peer list.

use crate::error::EldError;
use crate::hex_encoding::{decode_fixed_hex, encode_hex};
use serde::de::{Error as SerdeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// Peer id of a Tendermint node: 20 raw bytes.
///
/// Parsing accepts an optional `0x` / `0X` prefix and either hex case.
/// [`fmt::Display`] and serde output are lowercase hex with no prefix.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId {
    bytes: [u8; Self::LEN],
}

impl NodeId {
    /// Id length in bytes. Same length as a Tendermint node id.
    pub const LEN: usize = 20;

    /// Builds an id from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns [`EldError::ValidationError`] when `bytes` is not [`Self::LEN`] bytes.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, EldError> {
        if bytes.len() != Self::LEN {
            return Err(EldError::ValidationError {
                field: "node_id".to_string(),
                value: format!("{} bytes", bytes.len()),
                details: format!("node id must be {} bytes, got {}", Self::LEN, bytes.len()),
            });
        }
        let mut value = [0u8; Self::LEN];
        value.copy_from_slice(bytes);
        Ok(Self { bytes: value })
    }

    /// Builds an id from a Tendermint node id.
    ///
    /// # Errors
    ///
    /// Returns [`EldError::ValidationError`] when the Tendermint id is not [`Self::LEN`] bytes.
    pub fn from_tendermint(id: &tendermint::node::Id) -> Result<Self, EldError> {
        Self::from_slice(id.as_bytes())
    }

    /// Parses 40 hexadecimal digits. An optional `0x` / `0X` prefix is accepted.
    ///
    /// # Errors
    ///
    /// Returns [`EldError::ValidationError`] when `s` is not 20 bytes of hex.
    pub fn parse_hex(s: &str) -> Result<Self, EldError> {
        let bytes = decode_fixed_hex::<{ Self::LEN }>(s, "node id")?;
        Ok(Self { bytes })
    }

    /// Raw id bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; Self::LEN] {
        &self.bytes
    }

    /// Lowercase hex without a `0x` prefix (40 digits).
    #[must_use]
    pub fn to_hex(&self) -> String {
        encode_hex(&self.bytes)
    }
}

impl FromStr for NodeId {
    type Err = EldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_hex(s)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({self})")
    }
}

impl Hash for NodeId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bytes.hash(state);
    }
}

impl Serialize for NodeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NodeIdVisitor;

        impl Visitor<'_> for NodeIdVisitor {
            type Value = NodeId;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a 40-digit hex node id (optional 0x prefix)")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: SerdeError,
            {
                NodeId::parse_hex(value).map_err(SerdeError::custom)
            }
        }

        deserializer.deserialize_str(NodeIdVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn from_slice_rejects_the_wrong_length() {
        assert!(NodeId::from_slice(&[0u8; 19]).is_err());
        assert!(NodeId::from_slice(&[0u8; 21]).is_err());
        assert!(NodeId::from_slice(&[]).is_err());
    }

    #[test]
    fn from_tendermint_keeps_the_same_bytes() {
        let raw = [0xABu8; NodeId::LEN];
        let tm = tendermint::node::Id::new(raw);
        let id = NodeId::from_tendermint(&tm).expect("20-byte tendermint id");
        assert_eq!(id.as_bytes(), &raw);
        assert_eq!(id.to_hex(), hex::encode(raw));
    }

    #[test]
    fn parse_hex_accepts_uppercase_and_optional_prefix() {
        let upper = SAMPLE.to_ascii_uppercase();
        let id = NodeId::parse_hex(&upper).expect("upper");
        assert_eq!(id.to_hex(), SAMPLE);
        assert_eq!(NodeId::parse_hex(&format!("0x{upper}")).expect("0x"), id);
        assert_eq!(NodeId::parse_hex(&format!("0X{SAMPLE}")).expect("0X"), id);
    }

    #[test]
    fn parse_hex_rejects_the_wrong_length() {
        assert!(NodeId::parse_hex("00").is_err());
        assert!(NodeId::parse_hex("0x00").is_err());
        assert!(NodeId::parse_hex(&format!("{SAMPLE}00")).is_err());
    }

    #[test]
    fn display_and_serde_are_lowercase_hex_without_a_prefix() {
        let id = NodeId::parse_hex(SAMPLE).expect("sample");
        assert_eq!(id.to_string(), SAMPLE);
        let json = serde_json::to_string(&id).expect("ser");
        assert_eq!(json, format!("\"{SAMPLE}\""));
        let back: NodeId = serde_json::from_str(&json).expect("de");
        assert_eq!(back, id);
    }
}
