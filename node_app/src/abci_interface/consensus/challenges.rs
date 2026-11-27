use super::connection::ConsensusConnection;
use crate::app_state::AppState;
use crate::storage::traits::ConsensusConnectionStorage;
use eld_common::address::Address;
use eld_common::capacity_challenge::{compute_challenge_id, select_challenge_chunk_indices};
use eld_common::error::EldError;
use eld_common::NodeId;
use eld_tendermint_proto::content::CapacityChallenge;
use tracing::{info, warn};

pub(crate) type ChallengedCapacityProviderData = (
    Address,
    Option<[u8; 32]>,
    Option<[u8; 32]>,
    Option<u32>,
    NodeId,
);

impl<S> ConsensusConnection<S>
where
    S: ConsensusConnectionStorage,
{
    /// Returns true when this process is the epoch's active capacity validator
    /// (local capacity-validator **wallet** address matches on-chain selected address).
    pub(crate) fn should_this_node_send_challenges(&self, selected_validator: &Address) -> bool {
        let identity_guard = match self.local_identity.read() {
            Ok(guard) => guard,
            Err(e) => {
                warn!(
                    error = %e,
                    "Failed to read local identity; deferring capacity challenges"
                );
                return false;
            }
        };

        if identity_guard.matches_capacity_validator_wallet(selected_validator) {
            return true;
        }

        match identity_guard.capacity_validator_address {
            Some(local) => {
                info!(
                    local_capacity_validator = %local,
                    selected_capacity_validator = %selected_validator,
                    "Not the active capacity validator on this node; skipping capacity challenges"
                );
                false
            }
            None => {
                warn!(
                    "Local capacity validator wallet not configured; deferring capacity challenges"
                );
                false
            }
        }
    }

    pub(crate) fn build_challenged_capacity_provider_data(
        current_state: &AppState,
        challenged_capacity_provider_ids: &[Address],
    ) -> Vec<ChallengedCapacityProviderData> {
        challenged_capacity_provider_ids
            .iter()
            .filter_map(|provider_id| {
                current_state
                    .envelope
                    .capacity_validators
                    .iter()
                    .find(|cp| cp.address == *provider_id)
                    .map(|cp| {
                        (
                            *provider_id,
                            cp.merkle_root,
                            cp.seed,
                            cp.chunk_count,
                            cp.node_id,
                        )
                    })
            })
            .collect()
    }

    /// One `CapacityChallenge` per provider. Does not send them.
    pub(crate) fn generate_challenges_internal(
        providers_data: Vec<ChallengedCapacityProviderData>,
        epoch: i64,
        block_height: i64,
        validator_address: Address,
        chunks_per_challenge: usize,
    ) -> Result<Vec<CapacityChallenge>, EldError> {
        if providers_data.is_empty() {
            warn!(
                epoch = epoch,
                "No providers to challenge, skipping challenge generation"
            );
            return Ok(Vec::new());
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expiration_block = block_height + 100;

        let mut challenges = Vec::new();
        for (provider_id, merkle_root_opt, seed_opt, chunk_count_opt, node_id) in providers_data {
            let merkle_root = merkle_root_opt.ok_or_else(|| EldError::ValidationError {
                field: "merkle_root".to_string(),
                value: "none".to_string(),
                details: format!("Provider {provider_id} has no merkle root"),
            })?;

            let seed = seed_opt.ok_or_else(|| EldError::ValidationError {
                field: "seed".to_string(),
                value: "none".to_string(),
                details: format!("Provider {provider_id} has no seed"),
            })?;

            let chunk_count = chunk_count_opt.unwrap_or(0);
            if chunk_count == 0 {
                warn!(
                    provider_id = %provider_id,
                    "Provider has no chunks, skipping challenge"
                );
                continue;
            }

            let chunk_indices = select_challenge_chunk_indices(
                epoch,
                &provider_id,
                block_height,
                &validator_address,
                chunk_count,
                chunks_per_challenge,
            );
            let challenge_id = compute_challenge_id(
                &validator_address,
                &provider_id,
                block_height,
                &chunk_indices,
            );
            let chunk_indices = chunk_indices
                .into_iter()
                .map(|index| {
                    u64::try_from(index).map_err(|_| EldError::ValidationError {
                        field: "chunk_index".to_string(),
                        value: index.to_string(),
                        details: "chunk index does not fit in a content challenge".to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            info!(
                challenge_id = %challenge_id,
                provider_id = %provider_id,
                challenger = %validator_address,
                chunk_count = chunk_indices.len(),
                block_height = block_height,
                expiration_block = expiration_block,
                "Built capacity challenge"
            );
            challenges.push(CapacityChallenge {
                challenge_id: challenge_id.to_hex(),
                challenger: validator_address.as_bytes().to_vec(),
                provider: provider_id.as_bytes().to_vec(),
                chunk_indices,
                block_height,
                merkle_root: merkle_root.to_vec(),
                seed: seed.to_vec(),
                expiration_block,
                timestamp,
                node_id: node_id.as_bytes().to_vec(),
            });
        }

        Ok(challenges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::hybrid_storage::HybridStorage;

    #[test]
    fn one_provider_becomes_one_capacity_challenge() {
        let validator = Address::from_bytes([1; 20]);
        let provider = Address::from_bytes([2; 20]);
        let node_id = NodeId::from_slice(&[3; 20]).expect("node id");
        let challenges = ConsensusConnection::<HybridStorage>::generate_challenges_internal(
            vec![(provider, Some([9; 32]), Some([8; 32]), Some(4), node_id)],
            3,
            100,
            validator,
            2,
        )
        .expect("challenges");
        assert_eq!(challenges.len(), 1);
        let challenge = &challenges[0];
        assert_eq!(
            challenge.challenger.as_slice(),
            validator.as_bytes().as_slice()
        );
        assert_eq!(
            challenge.provider.as_slice(),
            provider.as_bytes().as_slice()
        );
        assert_eq!(challenge.block_height, 100);
        assert_eq!(challenge.merkle_root, vec![9; 32]);
        assert_eq!(challenge.seed, vec![8; 32]);
        assert_eq!(challenge.expiration_block, 200);
        assert_eq!(challenge.chunk_indices.len(), 2);
        assert!(!challenge.challenge_id.is_empty());
        assert_eq!(challenge.node_id, node_id.as_bytes());
    }
}
