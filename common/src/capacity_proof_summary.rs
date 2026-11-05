//! Signed outcome of one epoch's capacity challenges.
//!
//! The capacity validator signs [`CapacityProofSummary::signing_bytes`]. Wire and JSON
//! keep the signature as 128 hex characters with no `0x` prefix.

use ed25519_dalek::Verifier;

use crate::error::EldError;
use crate::hex_encoding::{decode_fixed_hex, encode_hex};
use crate::public_key::PublicKey;
use crate::Address;
use serde::de::{Error as SerdeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Ed25519 signature of the epoch's capacity validator over [`CapacityProofSummary::signing_bytes`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CapacityProofSignature {
    bytes: [u8; Self::LEN],
}

impl CapacityProofSignature {
    /// Ed25519 signature length.
    pub const LEN: usize = 64;

    /// Constructs from the raw 64 signature bytes.
    #[must_use]
    pub fn new(bytes: [u8; Self::LEN]) -> Self {
        Self { bytes }
    }

    /// Parses 128 hexadecimal digits. An optional `0x` prefix is accepted.
    ///
    /// # Errors
    ///
    /// Returns [`EldError::ValidationError`] when `text` is not a 64-byte hex signature.
    pub fn parse_hex(text: &str) -> Result<Self, EldError> {
        let bytes = decode_fixed_hex::<{ Self::LEN }>(text, "capacity proof signature")?;
        Ok(Self::new(bytes))
    }

    /// Raw signature bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; Self::LEN] {
        &self.bytes
    }

    /// Lowercase hex with no `0x` prefix.
    #[must_use]
    pub fn to_hex(&self) -> String {
        encode_hex(&self.bytes)
    }
}

impl fmt::Display for CapacityProofSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for CapacityProofSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapacityProofSignature({self})")
    }
}

impl Serialize for CapacityProofSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for CapacityProofSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SignatureVisitor;

        impl Visitor<'_> for SignatureVisitor {
            type Value = CapacityProofSignature;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a 128-digit hex Ed25519 signature")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: SerdeError,
            {
                CapacityProofSignature::parse_hex(value).map_err(SerdeError::custom)
            }
        }

        deserializer.deserialize_str(SignatureVisitor)
    }
}

/// What the capacity validator recorded for one challenged provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityProofOutcome {
    /// The provider's proof verified.
    Success {
        /// Challenged capacity provider.
        provider: Address,
    },
    /// The provider's proof was missing or did not verify.
    Failure {
        /// Challenged capacity provider.
        provider: Address,
    },
}

impl CapacityProofOutcome {
    /// The provider this outcome is about.
    #[must_use]
    pub fn provider(&self) -> Address {
        match self {
            Self::Success { provider } | Self::Failure { provider } => *provider,
        }
    }
}

/// One epoch of challenge outcomes, signed by that epoch's capacity validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityProofSummary {
    /// Epoch these outcomes belong to.
    pub epoch: i64,
    /// Capacity validator who selected the providers and signed this summary.
    pub validator: Address,
    /// One entry per challenged provider.
    pub outcomes: Vec<CapacityProofOutcome>,
    /// Signature over [`Self::signing_bytes`].
    pub signature: CapacityProofSignature,
}

impl CapacityProofSummary {
    /// Bytes the capacity validator signs.
    ///
    /// `CAPACITY_PROOF_SUMMARY`, the epoch as big-endian, the validator address hex,
    /// then each outcome as `S` or `F` plus that provider's address hex.
    #[must_use]
    pub fn signing_bytes(
        epoch: i64,
        validator: &Address,
        outcomes: &[CapacityProofOutcome],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"CAPACITY_PROOF_SUMMARY");
        bytes.extend_from_slice(&epoch.to_be_bytes());
        bytes.extend_from_slice(validator.hex().as_bytes());
        for outcome in outcomes {
            let (mark, provider) = match outcome {
                CapacityProofOutcome::Success { provider } => (b'S', provider),
                CapacityProofOutcome::Failure { provider } => (b'F', provider),
            };
            bytes.push(mark);
            bytes.extend_from_slice(provider.hex().as_bytes());
        }
        bytes
    }

    /// Whether `bytes` are the JSON encoding of a [`CapacityProofSummary`].
    #[must_use]
    pub fn is_capacity_proof_summary(bytes: &[u8]) -> bool {
        serde_json::from_slice::<Self>(bytes).is_ok()
    }

    /// Checks that `public_key` is this validator's key and signed [`Self::signing_bytes`].
    ///
    /// # Errors
    ///
    /// Returns [`EldError::ValidationError`] when the key does not match `validator`
    /// or the signature does not verify.
    pub fn verify(&self, public_key: &PublicKey) -> Result<(), EldError> {
        if !self.validator.is_from_public_key(public_key.as_bytes()) {
            return Err(EldError::ValidationError {
                field: "validator".to_string(),
                value: self.validator.to_string(),
                details: "capacity proof summary validator does not match the public key"
                    .to_string(),
            });
        }
        let signature = ed25519_dalek::Signature::from_bytes(self.signature.as_bytes());
        public_key
            .to_verifying_key()
            .verify(
                &Self::signing_bytes(self.epoch, &self.validator, &self.outcomes),
                &signature,
            )
            .map_err(|err| EldError::ValidationError {
                field: "signature".to_string(),
                value: self.signature.to_hex(),
                details: format!("capacity proof summary signature is not valid: {err}"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::public_key::PublicKey;

    fn sample_signature() -> CapacityProofSignature {
        CapacityProofSignature::new([0xAB; 64])
    }

    #[test]
    fn summary_keeps_epoch_validator_outcomes_and_signature() {
        let validator = Address::from_bytes([1; 20]);
        let provider = Address::from_bytes([2; 20]);
        let outcomes = vec![
            CapacityProofOutcome::Success { provider },
            CapacityProofOutcome::Failure {
                provider: Address::from_bytes([3; 20]),
            },
        ];
        let summary = CapacityProofSummary {
            epoch: 4,
            validator,
            outcomes: outcomes.clone(),
            signature: sample_signature(),
        };
        assert_eq!(summary.epoch, 4);
        assert_eq!(summary.validator, validator);
        assert_eq!(summary.outcomes[0].provider(), provider);
        assert_eq!(summary.signature, sample_signature());
        let json = serde_json::to_string(&summary).expect("json");
        let parsed: CapacityProofSummary = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed, summary);
    }

    #[test]
    fn signing_bytes_change_when_the_outcome_changes() {
        let validator = Address::from_bytes([1; 20]);
        let provider = Address::from_bytes([2; 20]);
        let success = CapacityProofSummary::signing_bytes(
            4,
            &validator,
            &[CapacityProofOutcome::Success { provider }],
        );
        let failure = CapacityProofSummary::signing_bytes(
            4,
            &validator,
            &[CapacityProofOutcome::Failure { provider }],
        );
        assert_ne!(success, failure);
    }

    #[test]
    fn signature_hex_rejects_the_wrong_length() {
        assert!(CapacityProofSignature::parse_hex("abcd").is_err());
    }

    #[test]
    fn verify_accepts_the_validators_signature() {
        use ed25519_dalek::{Signer, SigningKey};

        let key = SigningKey::from_bytes(&[4; 32]);
        let validator = Address::from_public_key(&key.verifying_key()).expect("address");
        let outcomes = vec![CapacityProofOutcome::Success {
            provider: Address::from_bytes([2; 20]),
        }];
        let signature = key.sign(&CapacityProofSummary::signing_bytes(
            4, &validator, &outcomes,
        ));
        let summary = CapacityProofSummary {
            epoch: 4,
            validator,
            outcomes,
            signature: CapacityProofSignature::new(signature.to_bytes()),
        };
        let public_key = PublicKey::from(key.verifying_key());
        summary.verify(&public_key).expect("signature");

        let mut tampered = summary.clone();
        tampered.epoch = 5;
        assert!(tampered.verify(&public_key).is_err());
    }

    #[test]
    fn json_bytes_are_recognized_as_a_summary() {
        let summary = CapacityProofSummary {
            epoch: 4,
            validator: Address::from_bytes([1; 20]),
            outcomes: vec![CapacityProofOutcome::Success {
                provider: Address::from_bytes([2; 20]),
            }],
            signature: sample_signature(),
        };
        let bytes = serde_json::to_vec(&summary).expect("json");
        assert!(CapacityProofSummary::is_capacity_proof_summary(&bytes));
        assert!(!CapacityProofSummary::is_capacity_proof_summary(
            b"{\"type\":\"Transfer\"}"
        ));
    }
}
