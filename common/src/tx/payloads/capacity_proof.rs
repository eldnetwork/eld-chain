//! A capacity proof submitted by the capacity validator.
//!
//! The sender is the validator. The payload carries the challenged node's signed
//! response. This node has checked that signature. It has not checked that the
//! chunk proofs are valid. The ABCI processor does that later.

use crate::capacity_proof::ChunkProof;
use crate::error::EldError;
use crate::tx::parts::HasSender;
use crate::Address;
use serde::{Deserialize, Serialize};

/// Transaction payload for one capacity challenge response.
///
/// `sender` is the capacity validator who received the response.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(try_from = "CapacityProofTxUnchecked")]
pub struct CapacityProofTx {
    /// Capacity validator. This is the transaction sender.
    pub sender: Address,
    /// Challenged capacity provider.
    pub capacity_provider: Address,
    /// Challenge this response answers.
    pub challenge_id: String,
    /// Block height of the challenge.
    pub block_height: i64,
    /// Chunk proofs from the challenged node.
    pub proofs: Vec<ChunkProof>,
    /// When the challenged node signed the response.
    pub generated_at: u64,
    /// Hex Ed25519 public key of the challenged node.
    pub provider_pubkey: String,
    /// Hex Ed25519 signature of the challenged node over the response.
    pub provider_signature: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapacityProofTxUnchecked {
    sender: Address,
    capacity_provider: Address,
    challenge_id: String,
    block_height: i64,
    proofs: Vec<ChunkProof>,
    generated_at: u64,
    provider_pubkey: String,
    provider_signature: String,
}

impl TryFrom<CapacityProofTxUnchecked> for CapacityProofTx {
    type Error = EldError;

    fn try_from(unchecked: CapacityProofTxUnchecked) -> Result<Self, Self::Error> {
        Self::new(
            unchecked.sender,
            unchecked.capacity_provider,
            unchecked.challenge_id,
            unchecked.block_height,
            unchecked.proofs,
            unchecked.generated_at,
            unchecked.provider_pubkey,
            unchecked.provider_signature,
        )
    }
}

fn validate_challenge_id(challenge_id: &str) -> Result<(), EldError> {
    if challenge_id.is_empty() {
        return Err(EldError::ValidationError {
            field: "challenge_id".to_string(),
            value: challenge_id.to_string(),
            details: "String cannot be empty".to_string(),
        });
    }
    const MAX_LEN: usize = 100;
    if challenge_id.len() > MAX_LEN {
        return Err(EldError::ValidationError {
            field: "challenge_id".to_string(),
            value: challenge_id.to_string(),
            details: format!("String exceeds maximum length of {MAX_LEN}"),
        });
    }
    let dangerous_chars = [
        '<', '>', '"', '\'', '&', ';', '|', '`', '$', '(', ')', '{', '}',
    ];
    for &ch in &dangerous_chars {
        if challenge_id.contains(ch) {
            return Err(EldError::ValidationError {
                field: "challenge_id".to_string(),
                value: challenge_id.to_string(),
                details: format!("String contains potentially dangerous character: {ch}"),
            });
        }
    }
    if challenge_id.chars().any(|c| c.is_control()) {
        return Err(EldError::ValidationError {
            field: "challenge_id".to_string(),
            value: challenge_id.to_string(),
            details: "String contains control characters".to_string(),
        });
    }
    Ok(())
}

impl CapacityProofTx {
    /// Builds a capacity-proof payload.
    ///
    /// Checks the challenge id, height, timestamps, and that the challenged
    /// node's public key and signature are present. Does not check that the
    /// signature or the chunk proofs are valid.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sender: Address,
        capacity_provider: Address,
        challenge_id: String,
        block_height: i64,
        proofs: Vec<ChunkProof>,
        generated_at: u64,
        provider_pubkey: String,
        provider_signature: String,
    ) -> Result<Self, EldError> {
        validate_challenge_id(&challenge_id)?;
        if block_height <= 0 {
            EldError::validation_error(
                "block_height",
                &block_height.to_string(),
                "Block height must be positive",
            )?;
        }
        const MIN_TIMESTAMP: u64 = 1_577_836_800;
        const MAX_TIMESTAMP: u64 = 4_102_444_800;
        if !(MIN_TIMESTAMP..=MAX_TIMESTAMP).contains(&generated_at) {
            EldError::validation_error(
                "generated_at",
                &generated_at.to_string(),
                &format!("generated_at must be between {MIN_TIMESTAMP} and {MAX_TIMESTAMP}"),
            )?;
        }
        if proofs.is_empty() {
            EldError::validation_error("proofs", "[]", "Capacity proof proofs must be non-empty")?;
        }
        if provider_pubkey.is_empty() {
            EldError::validation_error(
                "provider_pubkey",
                &provider_pubkey,
                "provider_pubkey must be non-empty hex",
            )?;
        }
        if provider_signature.is_empty() {
            EldError::validation_error(
                "provider_signature",
                &provider_signature,
                "provider_signature must be non-empty hex",
            )?;
        }
        Ok(Self {
            sender,
            capacity_provider,
            challenge_id,
            block_height,
            proofs,
            generated_at,
            provider_pubkey,
            provider_signature,
        })
    }
}

impl std::fmt::Display for CapacityProofTx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CapacityProofTx {{\n sender: {}\n capacity_provider: {}\n challenge_id: {}\n block_height: {}\n proofs: {}\n generated_at: {}\n }}",
            self.sender,
            self.capacity_provider,
            self.challenge_id,
            self.block_height,
            self.proofs.len(),
            self.generated_at
        )
    }
}

impl HasSender for CapacityProofTx {
    fn sender(&self) -> Address {
        self.sender
    }
}

#[cfg(test)]
mod tests {
    use super::CapacityProofTx;
    use crate::capacity_proof::{ChunkProof, SlotState};
    use crate::constants::tx_type::TX_TYPE_CAPACITY_PROOF;
    use crate::tx::{Payload, PayloadInner, Tx, TxPublicKey, TxSig};
    use crate::Address;
    use ed25519_dalek::SigningKey;

    fn sample() -> CapacityProofTx {
        CapacityProofTx::new(
            Address::parse_hex_str("0x1111111111111111111111111111111111111111").unwrap(),
            Address::parse_hex_str("0x2222222222222222222222222222222222222222").unwrap(),
            "abc".to_string(),
            1,
            vec![ChunkProof {
                chunk_index: 0,
                chunk_data: vec![1],
                chunk_hash: [2u8; 32],
                merkle_proof: vec![],
                slot_state: SlotState::Proof,
            }],
            1_600_000_000,
            "03".repeat(32),
            "04".repeat(64),
        )
        .unwrap()
    }

    #[test]
    fn round_trips_inside_the_transfer_envelope() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let payload = Payload::new(sample());
        assert_eq!(payload.r#type, TX_TYPE_CAPACITY_PROOF);
        let tx = Tx::new(
            1u32,
            payload,
            TxPublicKey::from(signing_key.verifying_key()),
        );
        let json = serde_json::to_string(&tx).unwrap();
        let decoded: Tx = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            decoded.payload.inner,
            PayloadInner::CapacityProof(_)
        ));
        assert_eq!(decoded.payload.r#type, TX_TYPE_CAPACITY_PROOF);
        assert_eq!(decoded.sig, TxSig::empty());
    }

    #[test]
    fn rejects_a_verified_proof_shape() {
        let mut value = serde_json::to_value(sample()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("verified_at_block".into(), 2.into());
        object.insert("verified_at_timestamp".into(), 1_600_000_000u64.into());
        object.insert("failed".into(), false.into());
        assert!(serde_json::from_value::<CapacityProofTx>(value).is_err());
    }
}
