//! `DeliverTx` for a capacity-proof summary transaction already in the block.
//!
//! `sender` is the capacity validator. Every node applies the same transaction.
//! Local challenge results are not read.

use super::capacity_challenge_binding::record_failed_capacity_proof;
use crate::abci_interface::ConsensusConnection;
use crate::app_state::AccountWithCadoHash;
use crate::errors::response_deliver_tx_error_validation_failed;
use crate::storage::traits::ConsensusConnectionStorage;
use abci::types::{Event, ResponseDeliverTx};
use eld_common::account::Account;
use eld_common::cado::{CADOMetadata, CadoBody, CadoPath, CadoPathKey, CadoType};
use eld_common::capacity_proof_summary::{CapacityProofOutcome, CapacityProofSummary};
use eld_common::coin::Coin;
use eld_common::constants::tx_type;
use eld_common::nonce::Nonce;
use eld_common::tx::create_event_attribute;
use eld_common::tx::CapacityProofSummaryTx;
use eld_common::validator::EpochRecord;
use tracing::{error, warn};

pub async fn process_capacity_proof_summary<S>(
    connection: &ConsensusConnection<S>,
    tx: CapacityProofSummaryTx,
) -> ResponseDeliverTx
where
    S: ConsensusConnectionStorage,
{
    let summary = tx.summary();
    let protocol = connection.protocol_constants().clone();
    let reward_amount = protocol.verified_proof_reward_base_amount.amount();
    let reward_coin = match Coin::new(reward_amount) {
        Ok(coin) => coin,
        Err(err) => return response_deliver_tx_error_validation_failed(err.to_string()),
    };

    let mut current_state_lock = connection
        .current_state
        .lock()
        .expect("Failed to acquire current_state lock");
    let current_state = current_state_lock
        .as_mut()
        .expect("current_state lock is None");

    let current_epoch = current_state.envelope.current_epoch;
    if let Err(reason) = epoch_in_window(summary.epoch, current_epoch) {
        warn!(reason = %reason, "capacity proof summary rejected");
        return response_deliver_tx_error_validation_failed(reason);
    }

    let epoch_record = match current_state
        .envelope
        .get_epoch_record(&*connection.storage, summary.epoch)
    {
        Ok(Some(record)) => record,
        Ok(None) => {
            let reason = format!(
                "capacity proof summary rejected: no epoch record for epoch {}",
                summary.epoch
            );
            warn!(reason = %reason, "capacity proof summary rejected");
            return response_deliver_tx_error_validation_failed(reason);
        }
        Err(err) => return response_deliver_tx_error_validation_failed(err.to_string()),
    };

    let Some(active) = epoch_record.active_capacity_validator.as_ref() else {
        let reason = format!(
            "capacity proof summary rejected: no active_capacity_validator for epoch {}",
            summary.epoch
        );
        warn!(reason = %reason, "capacity proof summary rejected");
        return response_deliver_tx_error_validation_failed(reason);
    };
    if active.validator_address != summary.validator || active.epoch != summary.epoch {
        let reason = format!(
            "capacity proof summary rejected: signer {} is not active_capacity_validator {} for epoch {}",
            summary.validator, active.validator_address, summary.epoch
        );
        warn!(reason = %reason, "capacity proof summary rejected");
        return response_deliver_tx_error_validation_failed(reason);
    }

    let Some(public_key) = current_state
        .envelope
        .capacity_validators
        .iter()
        .find(|validator| validator.address == summary.validator)
        .map(|validator| validator.public_key)
    else {
        let reason = format!(
            "capacity proof summary rejected: validator {} has no registered public key",
            summary.validator
        );
        warn!(reason = %reason, "capacity proof summary rejected");
        return response_deliver_tx_error_validation_failed(reason);
    };
    if let Err(err) = summary.verify(&public_key) {
        warn!(reason = %err, "capacity proof summary rejected: signature");
        return response_deliver_tx_error_validation_failed(err.to_string());
    }

    if let Err(reason) = outcomes_match_epoch(&summary, &epoch_record) {
        warn!(reason = %reason, "capacity proof summary rejected");
        return response_deliver_tx_error_validation_failed(reason);
    }

    for outcome in &summary.outcomes {
        let key = outcome_key(summary.epoch, outcome);
        match (
            current_state
                .envelope
                .is_verified_proof_challenge_rewarded(&*connection.storage, &key),
            current_state
                .envelope
                .is_verified_proof_challenge_failed(&*connection.storage, &key),
        ) {
            (Ok(false), Ok(false)) => {}
            (Ok(true), _) | (_, Ok(true)) => {
                let reason = format!(
                    "capacity proof summary rejected: outcome already applied for {}",
                    outcome.provider()
                );
                warn!(reason = %reason, "capacity proof summary rejected");
                return response_deliver_tx_error_validation_failed(reason);
            }
            (Err(err), _) | (_, Err(err)) => {
                return response_deliver_tx_error_validation_failed(err.to_string());
            }
        }
    }

    let mut events = Vec::new();
    for outcome in &summary.outcomes {
        let provider = outcome.provider();
        let key = outcome_key(summary.epoch, outcome);
        match outcome {
            CapacityProofOutcome::Success { .. } => {
                let recipient_str = provider.to_string();
                let recipient_path =
                    CadoPath::new(CadoType::Account, CadoPathKey::Address(provider))
                        .expect("Failed to create recipient CadoPath");
                let recipient_account_with_hash = match current_state
                    .envelope
                    .get_account_from_cado(&*connection.storage, &recipient_path)
                {
                    Some(account_with_hash) => account_with_hash,
                    None => {
                        warn!(
                            "Recipient account not found, initializing: {}",
                            recipient_str
                        );
                        AccountWithCadoHash {
                            account: Account::new(provider, Coin::zero(), Nonce::new(Nonce::ZERO)),
                            hash: [0; 32],
                        }
                    }
                };
                let recipient_account = recipient_account_with_hash.account;
                // MINT TO RECIPIENT!
                let updated_recipient = match recipient_account.balance() + reward_coin {
                    Ok(balance) => Account::new(
                        *recipient_account.address(),
                        balance,
                        recipient_account.nonce(),
                    ),
                    Err(e) => {
                        error!("Failed to add reward to recipient balance: {}", e);
                        return response_deliver_tx_error_validation_failed(e.to_string());
                    }
                };
                let recipient_serialized = bincode::serialize(&updated_recipient)
                    .expect("Failed to serialize updated recipient");
                let recipient_meta = CADOMetadata::new(CadoType::Account, &recipient_str);
                let recipient_cado = CadoBody::mutable_updated(
                    recipient_account_with_hash.hash,
                    recipient_serialized,
                    recipient_meta,
                );
                current_state
                    .envelope
                    .update_cado_cache(recipient_path, recipient_cado);
                extend_registration(
                    current_state,
                    provider,
                    protocol.registration_duration_blocks,
                );
                current_state
                    .envelope
                    .verified_proof_rewarded_cache
                    .insert(key);
                events.push(summary_event(
                    &summary,
                    provider,
                    "success",
                    None,
                    Some(reward_amount),
                ));
                events.push(Event {
                    r#type: tx_type::TX_TYPE_TRANSFER.into(),
                    attributes: vec![
                        create_event_attribute("from".into(), "rewards_pool".into()),
                        create_event_attribute("to".into(), provider.to_string()),
                        create_event_attribute("amount".into(), reward_amount.to_string()),
                    ],
                });
            }
            CapacityProofOutcome::Failure { .. } => {
                let (failure_count, slashed) = record_failed_capacity_proof(
                    &mut current_state.envelope,
                    provider,
                    protocol.failed_proofs_before_slash,
                );
                current_state
                    .envelope
                    .failed_proof_counted_cache
                    .insert(key);
                events.push(summary_event(
                    &summary,
                    provider,
                    "failure",
                    Some((failure_count, slashed)),
                    None,
                ));
            }
        }
    }

    ResponseDeliverTx {
        code: 0,
        log: "Capacity proof summary applied".to_string(),
        events,
        ..Default::default()
    }
}

fn epoch_in_window(epoch: i64, current_epoch: i64) -> Result<(), String> {
    if epoch < 0 || current_epoch < 0 {
        return Err("capacity proof summary epoch is invalid".to_string());
    }
    if epoch == current_epoch || (current_epoch > 0 && epoch == current_epoch - 1) {
        Ok(())
    } else {
        Err(format!(
            "capacity proof summary epoch {epoch} is outside the current epoch {current_epoch}"
        ))
    }
}

fn outcomes_match_epoch(
    summary: &CapacityProofSummary,
    record: &EpochRecord,
) -> Result<(), String> {
    let challenged = record
        .active_capacity_validator
        .as_ref()
        .map(|active| active.challenged_providers.as_slice())
        .unwrap_or(&[]);
    let mut seen = Vec::new();
    for outcome in &summary.outcomes {
        let provider = outcome.provider();
        if seen.contains(&provider) {
            return Err(format!(
                "capacity proof summary rejected: provider {provider} is listed twice"
            ));
        }
        seen.push(provider);
        if !challenged.contains(&provider) {
            return Err(format!(
                "capacity proof summary rejected: provider {provider} was not challenged in epoch {}",
                summary.epoch
            ));
        }
    }
    Ok(())
}

fn outcome_key(epoch: i64, outcome: &CapacityProofOutcome) -> String {
    format!("summary:{epoch}:{}", outcome.provider().hex())
}

fn extend_registration(
    current_state: &mut crate::app_state::AppState,
    provider: eld_common::Address,
    registration_duration_blocks: u64,
) {
    let current_block = (current_state.envelope.block_height + 1) as u64;
    for validator in current_state.envelope.capacity_validators.iter_mut() {
        if validator.address == provider && validator.registered_block != 0 {
            validator.registration_duration = current_block
                .saturating_sub(validator.registered_block)
                + registration_duration_blocks;
            break;
        }
    }
}

fn summary_event(
    summary: &CapacityProofSummary,
    provider: eld_common::Address,
    outcome: &str,
    failure: Option<(u32, bool)>,
    reward: Option<u128>,
) -> Event {
    let mut attributes = vec![
        create_event_attribute("epoch".into(), summary.epoch.to_string()),
        create_event_attribute("validator".into(), summary.validator.to_string()),
        create_event_attribute("capacity_provider".into(), provider.to_string()),
        create_event_attribute("outcome".into(), outcome.into()),
    ];
    if let Some((failure_count, slashed)) = failure {
        attributes.push(create_event_attribute(
            "failure_count".into(),
            failure_count.to_string(),
        ));
        attributes.push(create_event_attribute(
            "slashed".into(),
            slashed.to_string(),
        ));
    }
    if let Some(reward) = reward {
        attributes.push(create_event_attribute("reward".into(), reward.to_string()));
    }
    Event {
        r#type: "CapacityProofSummary".into(),
        attributes,
    }
}
