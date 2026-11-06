//! `DeliverTx` for a capacity-proof transaction already in the block.
//!
//! Every node applies the same transaction. The proof is checked against the
//! epoch record, then the provider reward is written into the current app state.

use super::verified_proof_processor::{bind_verified_proof, proof_epoch_in_window};
use crate::abci_interface::ConsensusConnection;
use crate::capacity::challenge_validator::validate_challenge_proof;
use crate::errors::response_deliver_tx_error_validation_failed;
use crate::storage::traits::ConsensusConnectionStorage;
use abci::types::{Event, ResponseDeliverTx};
use eld_common::coin::Coin;
use eld_common::constants::tx_type;
use eld_common::tx::{create_event_attribute, CapacityProofTx};
use tracing::warn;

pub async fn process_capacity_proof_tx<S>(
    connection: &ConsensusConnection<S>,
    tx: CapacityProofTx,
) -> ResponseDeliverTx
where
    S: ConsensusConnectionStorage,
{
    let protocol = connection.protocol_constants().clone();
    let reward_amount = protocol.verified_proof_reward_base_amount.amount();
    let reward_coin = match Coin::new(reward_amount) {
        Ok(coin) => coin,
        Err(err) => return response_deliver_tx_error_validation_failed(err.to_string()),
    };
    let registration_duration_blocks = protocol.registration_duration_blocks;

    let provider = tx.capacity_provider;
    let sender = tx.sender;
    let challenge_id = tx.challenge_id.clone();

    let mut current_state_lock = connection
        .current_state
        .lock()
        .expect("Failed to acquire current_state lock");
    let current_state = current_state_lock
        .as_mut()
        .expect("current_state lock is None");

    match current_state
        .envelope
        .is_verified_proof_challenge_rewarded(&*connection.storage, &challenge_id)
    {
        Ok(true) => {
            let reason =
                format!("capacity proof rejected: challenge_id already rewarded: {challenge_id}");
            warn!(reason = %reason, "capacity proof rejected");
            return response_deliver_tx_error_validation_failed(reason);
        }
        Ok(false) => {}
        Err(err) => return response_deliver_tx_error_validation_failed(err.to_string()),
    }
    match current_state
        .envelope
        .is_verified_proof_challenge_failed(&*connection.storage, &challenge_id)
    {
        Ok(true) => {
            let reason = format!(
                "capacity proof rejected: challenge_id already counted as a failure: {challenge_id}"
            );
            warn!(reason = %reason, "capacity proof rejected");
            return response_deliver_tx_error_validation_failed(reason);
        }
        Ok(false) => {}
        Err(err) => return response_deliver_tx_error_validation_failed(err.to_string()),
    }

    let current_epoch = current_state.envelope.current_epoch;
    let epoch_record =
        match proof_epoch_in_window(tx.block_height, current_epoch, protocol.blocks_per_epoch) {
            Ok(proof_epoch) => {
                match current_state
                    .envelope
                    .get_epoch_record(&*connection.storage, proof_epoch)
                {
                    Ok(record) => record,
                    Err(err) => {
                        return response_deliver_tx_error_validation_failed(err.to_string())
                    }
                }
            }
            Err(reason) => {
                warn!(reason = %reason, "capacity proof rejected");
                return response_deliver_tx_error_validation_failed(reason);
            }
        };

    let binding = match bind_verified_proof(
        epoch_record.as_ref(),
        current_epoch,
        sender,
        provider,
        &challenge_id,
        tx.block_height,
        &protocol,
    ) {
        Ok(binding) => binding,
        Err(reason) => {
            warn!(reason = %reason, "capacity proof rejected");
            return response_deliver_tx_error_validation_failed(reason);
        }
    };

    if let Err(err) = eld_common::capacity_proof::verify_capacity_challenge_response(
        &challenge_id,
        &binding.challenged_provider_id,
        &binding.validator_address,
        tx.block_height,
        &tx.proofs,
        tx.generated_at,
        &tx.provider_pubkey,
        &tx.provider_signature,
    ) {
        let reason = format!("capacity proof rejected: provider signature invalid: {err}");
        warn!(reason = %reason, "capacity proof rejected");
        return response_deliver_tx_error_validation_failed(reason);
    }

    let proof_check = validate_challenge_proof(
        &tx.proofs,
        &binding.expected_merkle_root,
        &binding.expected_indices,
    );
    if !proof_check.is_valid {
        // TODO: penalize the capacity provider for an invalid proof.
        let reason = format!(
            "capacity proof rejected: proof validation failed ({} error(s))",
            proof_check.errors.len()
        );
        warn!(reason = %reason, "capacity proof rejected");
        return response_deliver_tx_error_validation_failed(reason);
    }

    if let Err(err) =
        connection
            .reward_manager
            .credit_verified_proof_reward(current_state, provider, reward_coin)
    {
        return response_deliver_tx_error_validation_failed(err.to_string());
    }

    let current_block = (current_state.envelope.block_height + 1) as u64;
    for validator in current_state.envelope.capacity_validators.iter_mut() {
        if validator.address == provider && validator.registered_block != 0 {
            validator.registration_duration = current_block
                .saturating_sub(validator.registered_block)
                + registration_duration_blocks;
            break;
        }
    }

    current_state
        .envelope
        .verified_proof_rewarded_cache
        .insert(challenge_id.clone());

    let events = vec![
        Event {
            r#type: tx_type::TX_TYPE_CAPACITY_PROOF.into(),
            attributes: vec![
                create_event_attribute("sender".into(), sender.to_string()),
                create_event_attribute("capacity_provider".into(), provider.to_string()),
                create_event_attribute("challenge_id".into(), challenge_id),
                create_event_attribute("block_height".into(), tx.block_height.to_string()),
                create_event_attribute("reward".into(), reward_amount.to_string()),
            ],
        },
        Event {
            r#type: tx_type::TX_TYPE_TRANSFER.into(),
            attributes: vec![
                create_event_attribute("from".into(), "rewards_pool".into()),
                create_event_attribute("to".into(), provider.to_string()),
                create_event_attribute("amount".into(), reward_amount.to_string()),
            ],
        },
    ];

    ResponseDeliverTx {
        code: 0,
        log: format!("capacity proof processed; minted {reward_amount} to {provider}"),
        events,
        ..Default::default()
    }
}
