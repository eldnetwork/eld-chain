//! Content-connection methods. Peer bytes arrive here from the Tendermint node.
//!
//! Storage, proofs, and the missing-content tracker are the app's own. This
//! file does not call the app's p2p message handlers.

use std::time::UNIX_EPOCH;

use eld_common::address::Address;
use eld_common::constants::pinboard::MAX_CHUNK_SIZE;
use eld_common::{CapacityMerkleRoot, ChallengeId};
use eld_tendermint_proto::content::{self, ContentPushAck, ContentResponse, Message};
use prost::Message as ProstMessage;
use sha2::{Digest, Sha256};
use tracing::error;

use crate::capacity::capacity_manager::CapacityProofGenerationParams;
use crate::capacity::challenge_validator::validate_challenge_proof;

use super::connection::ContentConnection;
use super::proof::{chunk_proof_from_proto, chunk_proof_to_proto};
use super::temp_storage::ContentTempStorage;

/// Pinboard messages stop at 1 MiB. The Tendermint content channel uses the same cap.
const MAX_BLOB_BYTES: usize = 1024 * 1024;

/// Same short life as a pinboard upload's temp copy.
const TEMP_BLOB_TTL_SECS: u64 = 15 * 60;

impl<B> ContentConnection<B>
where
    B: ContentTempStorage,
{
    pub(super) async fn answer(&self, message_bytes: &[u8]) -> Vec<u8> {
        let Ok(message) = Message::decode(message_bytes) else {
            return Vec::new();
        };
        let Some(sum) = message.sum else {
            return Vec::new();
        };
        let reply = match sum {
            content::message::Sum::ContentPush(msg) => self.handle_push(&msg).await,
            content::message::Sum::ContentRequest(msg) => self.handle_request(&msg).await,
            content::message::Sum::CapacityChallenge(msg) => self.handle_challenge(&msg).await,
            content::message::Sum::ContentPushAck(msg) => self.handle_push_ack(&msg),
            content::message::Sum::ContentResponse(msg) => self.handle_response(&msg).await,
            content::message::Sum::CapacityChallengeResponse(msg) => {
                self.handle_challenge_response(&msg).await
            }
            content::message::Sum::Announce(msg) => self.handle_announce(&msg).await,
            content::message::Sum::CapacityProofSummaryQuery(msg) => {
                self.signed_summary_for_height(msg.height).await
            }
            content::message::Sum::CapacityProofSummary(_) => None,
        };
        match reply {
            Some(message) => message.encode_to_vec(),
            None => Vec::new(),
        }
    }

    async fn handle_push(&self, msg: &content::ContentPush) -> Option<Message> {
        if msg.blob.is_empty() || msg.blob.len() > MAX_BLOB_BYTES {
            return None;
        }
        if !blob_matches_content_id(&msg.content_id, &msg.blob) {
            return None;
        }
        if let Err(err) = self.store_blob(&msg.content_id, &msg.blob).await {
            error!(
                content_id = %msg.content_id,
                error = %err,
                "content push was not stored"
            );
            return None;
        }
        Some(Message {
            sum: Some(content::message::Sum::ContentPushAck(ContentPushAck {
                request_id: msg.request_id,
                content_id: msg.content_id.clone(),
                provider: self.capacity.config().provider_id.as_bytes().to_vec(),
            })),
        })
    }

    async fn handle_request(&self, msg: &content::ContentRequest) -> Option<Message> {
        let blob = match self.blobs.read_temp_blob(&msg.content_id) {
            Ok(Some(blob)) => Some(blob),
            Ok(None) => match self.capacity.get_content_from_slots(&msg.content_id).await {
                Ok(blob) => blob,
                Err(err) => {
                    error!(
                        content_id = %msg.content_id,
                        error = %err,
                        "content read failed"
                    );
                    None
                }
            },
            Err(err) => {
                error!(
                    content_id = %msg.content_id,
                    error = %err,
                    "temp blob read failed"
                );
                None
            }
        }?;
        Some(Message {
            sum: Some(content::message::Sum::ContentResponse(ContentResponse {
                request_id: msg.request_id,
                content_id: msg.content_id.clone(),
                blob,
            })),
        })
    }

    async fn handle_challenge(&self, msg: &content::CapacityChallenge) -> Option<Message> {
        let params = challenge_params(msg)?;
        let proof = match self.capacity.generate_capacity_proof(params).await {
            Ok(proof) => proof,
            Err(err) => {
                error!(
                    challenge_id = %msg.challenge_id,
                    error = %err,
                    "capacity proof was not generated"
                );
                return None;
            }
        };
        let (provider_pubkey, provider_signature) =
            match self.capacity.sign_capacity_challenge_response(&proof).await {
                Ok(signed) => signed,
                Err(err) => {
                    error!(
                        challenge_id = %proof.challenge_id,
                        error = %err,
                        "capacity proof was not signed"
                    );
                    return None;
                }
            };
        let provider_pubkey = decode_hex_bytes(&provider_pubkey, 32)?;
        let provider_signature = decode_hex_bytes(&provider_signature, 64)?;
        let proofs = proof
            .proofs
            .iter()
            .map(chunk_proof_to_proto)
            .collect::<Option<Vec<_>>>()?;
        Some(Message {
            sum: Some(content::message::Sum::CapacityChallengeResponse(
                content::CapacityChallengeResponse {
                    challenge_id: proof.challenge_id,
                    provider: proof.provider_id.as_bytes().to_vec(),
                    challenger: proof.challenger.as_bytes().to_vec(),
                    block_height: proof.block_height,
                    proofs,
                    generated_at: proof.generated_at,
                    provider_pubkey,
                    provider_signature,
                },
            )),
        })
    }

    fn handle_push_ack(&self, msg: &content::ContentPushAck) -> Option<Message> {
        if msg.provider.len() != 20 {
            error!(
                content_id = %msg.content_id,
                "content receipt provider is not 20 bytes"
            );
            return None;
        }
        let mut receipts = self.receipts.lock().unwrap_or_else(|err| err.into_inner());
        let already = receipts.iter().any(|receipt| {
            receipt.request_id == msg.request_id
                && receipt.content_id == msg.content_id
                && receipt.provider == msg.provider
        });
        if !already {
            receipts.push(super::connection::ContentReceipt {
                request_id: msg.request_id,
                content_id: msg.content_id.clone(),
                provider: msg.provider.clone(),
            });
        }
        None
    }

    async fn handle_response(&self, msg: &content::ContentResponse) -> Option<Message> {
        if msg.blob.len() > MAX_BLOB_BYTES || !blob_matches_content_id(&msg.content_id, &msg.blob) {
            error!(content_id = %msg.content_id, "content response hash did not match");
            return None;
        }
        if let Err(err) = self.store_blob(&msg.content_id, &msg.blob).await {
            error!(
                content_id = %msg.content_id,
                error = %err,
                "content response was not stored"
            );
            return None;
        }
        if let Err(err) = self
            .missing
            .lock()
            .unwrap_or_else(|lock_err| lock_err.into_inner())
            .mark_content_synced_by_content_id(&msg.content_id)
        {
            error!(
                content_id = %msg.content_id,
                error = %err,
                "missing content was not cleared"
            );
        }
        None
    }

    async fn handle_challenge_response(
        &self,
        msg: &content::CapacityChallengeResponse,
    ) -> Option<Message> {
        let challenger = address_from(&msg.challenger)?;
        let provider = address_from(&msg.provider)?;
        let is_local = self
            .local_identity
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .matches_capacity_validator_wallet(&challenger);
        if !is_local || !self.is_active_capacity_validator(&challenger) {
            return None;
        }
        let proofs = msg
            .proofs
            .iter()
            .map(chunk_proof_from_proto)
            .collect::<Option<Vec<_>>>()?;
        let provider_pubkey = hex::encode(&msg.provider_pubkey);
        let provider_signature = hex::encode(&msg.provider_signature);
        if let Err(err) = eld_common::capacity_proof::verify_capacity_challenge_response(
            &msg.challenge_id,
            &provider,
            &challenger,
            msg.block_height,
            &proofs,
            msg.generated_at,
            &provider_pubkey,
            &provider_signature,
        ) {
            error!(
                challenge_id = %msg.challenge_id,
                error = %err,
                "capacity proof signature was rejected"
            );
            return None;
        }
        let Some(root) = self.provider_merkle_root(&provider) else {
            error!(
                challenge_id = %msg.challenge_id,
                provider = %provider,
                "capacity provider has no merkle root"
            );
            return None;
        };
        let indices: Vec<usize> = proofs.iter().map(|proof| proof.chunk_index).collect();
        let checked = validate_challenge_proof(&proofs, &root, &indices);
        if checked.is_valid {
            match crate::node_identity::resolve_capacity_validator_wallet_name_from_env() {
                Ok(wallet_name) => {
                    if let Err(err) = self
                        .cli
                        .submit_capacity_proof(
                            wallet_name,
                            provider,
                            msg.challenge_id.clone(),
                            msg.block_height,
                            proofs.clone(),
                            msg.generated_at,
                            provider_pubkey,
                            provider_signature,
                        )
                        .await
                    {
                        error!(
                            challenge_id = %msg.challenge_id,
                            error = %err,
                            "capacity proof transaction was not submitted"
                        );
                    }
                }
                Err(err) => {
                    error!(
                        challenge_id = %msg.challenge_id,
                        error = %err,
                        "capacity proof transaction was not submitted"
                    );
                }
            }
        }
        let outcome = if checked.is_valid {
            super::connection::ChallengeOutcome::Verified {
                challenge_id: msg.challenge_id.clone(),
                provider: msg.provider.clone(),
            }
        } else {
            super::connection::ChallengeOutcome::Rejected {
                challenge_id: msg.challenge_id.clone(),
                provider: msg.provider.clone(),
                reason: checked.errors.join("; "),
            }
        };
        let mut outcomes = self
            .challenge_outcomes
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if !outcomes.iter().any(|saved| same_outcome(saved, &outcome)) {
            outcomes.push(outcome);
        }
        None
    }

    async fn signed_summary_for_height(&self, height: i64) -> Option<Message> {
        if height < 0 {
            return None;
        }
        let blocks_per_epoch = self
            .capacity
            .protocol_handle()
            .get()
            .map(|constants| constants.blocks_per_epoch)
            .unwrap_or(0);
        if blocks_per_epoch <= 0 {
            return None;
        }
        self.signed_summary(height / blocks_per_epoch).await
    }

    async fn signed_summary(&self, epoch: i64) -> Option<Message> {
        let summary = {
            let summaries = self
                .signed_summaries
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            summaries.get(&epoch)?.clone()
        };
        let nonce = self.next_validator_nonce(summary.validator)?;
        let bytes = match self
            .capacity
            .capacity_proof_summary_tx_bytes(&summary, nonce)
            .await
        {
            Ok(bytes) => bytes,
            Err(err) => {
                error!(
                    epoch,
                    error = %err,
                    "capacity proof summary transaction was not signed"
                );
                return None;
            }
        };
        Some(Message {
            sum: Some(content::message::Sum::CapacityProofSummary(
                content::CapacityProofSummaryBytes {
                    epoch,
                    summary: bytes,
                },
            )),
        })
    }

    fn next_validator_nonce(
        &self,
        validator: eld_common::Address,
    ) -> Option<eld_common::nonce::Nonce> {
        use eld_common::account::Account;
        use eld_common::cado::{CadoPath, CadoPathKey, CadoType};
        use eld_common::validation::safe_deserialize_account_data;

        let path = CadoPath::new(CadoType::Account, CadoPathKey::Address(validator)).ok()?;
        let state = self
            .committed_state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let cado = state
            .envelope
            .committed_cado_cache
            .get(path.as_str().as_bytes())?;
        let account: Account =
            safe_deserialize_account_data(cado.data(), "capacity validator account").ok()?;
        account.nonce().next()
    }

    async fn handle_announce(&self, msg: &content::Announce) -> Option<Message> {
        if self.have_blob(&msg.content_id).await {
            return None;
        }
        if let Err(err) = self
            .missing
            .lock()
            .unwrap_or_else(|lock_err| lock_err.into_inner())
            .upsert_missing_content_by_content_id(&msg.content_id)
        {
            error!(
                content_id = %msg.content_id,
                error = %err,
                "announce was not recorded"
            );
        }
        None
    }

    async fn have_blob(&self, content_id: &str) -> bool {
        match self.blobs.read_temp_blob(content_id) {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(err) => {
                error!(content_id, error = %err, "temp blob read failed");
            }
        }
        match self.capacity.get_content_from_slots(content_id).await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(err) => {
                error!(content_id, error = %err, "slot read failed");
                false
            }
        }
    }

    async fn store_blob(
        &self,
        content_id: &str,
        blob: &[u8],
    ) -> Result<(), eld_common::error::EldError> {
        match self.capacity.get_content_from_slots(content_id).await {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(err) => return Err(err),
        }
        let chunks: Vec<Vec<u8>> = blob
            .chunks(MAX_CHUNK_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();
        if self
            .capacity
            .store_content_chunks(content_id.to_owned(), chunks)
            .await
            .is_ok()
        {
            return Ok(());
        }
        let expires_at = UNIX_EPOCH
            .elapsed()
            .map_err(|err| eld_common::error::EldError::StorageError {
                operation: "content_temp_blob_expiry".to_owned(),
                details: err.to_string(),
            })?
            .as_secs()
            .saturating_add(TEMP_BLOB_TTL_SECS);
        self.blobs.write_temp_blob(content_id, blob, expires_at)
    }

    fn is_active_capacity_validator(&self, challenger: &Address) -> bool {
        let state = self
            .committed_state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        state
            .envelope
            .active_capacity_validator
            .as_ref()
            .is_some_and(|validator| validator.validator_address == *challenger)
    }

    fn provider_merkle_root(&self, provider: &Address) -> Option<CapacityMerkleRoot> {
        let state = self
            .committed_state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        state
            .envelope
            .capacity_validators
            .iter()
            .find(|info| info.address == *provider)
            .and_then(|info| info.merkle_root)
            .map(CapacityMerkleRoot::new)
    }
}

fn same_outcome(
    saved: &super::connection::ChallengeOutcome,
    incoming: &super::connection::ChallengeOutcome,
) -> bool {
    match (saved, incoming) {
        (
            super::connection::ChallengeOutcome::Verified {
                challenge_id,
                provider,
            },
            super::connection::ChallengeOutcome::Verified {
                challenge_id: next_id,
                provider: next_provider,
            },
        ) => challenge_id == next_id && provider == next_provider,
        (
            super::connection::ChallengeOutcome::Rejected {
                challenge_id,
                provider,
                reason,
            },
            super::connection::ChallengeOutcome::Rejected {
                challenge_id: next_id,
                provider: next_provider,
                reason: next_reason,
            },
        ) => challenge_id == next_id && provider == next_provider && reason == next_reason,
        _ => false,
    }
}

fn challenge_params(msg: &content::CapacityChallenge) -> Option<CapacityProofGenerationParams> {
    let challenger = address_from(&msg.challenger)?;
    let provider = address_from(&msg.provider)?;
    let merkle_root: [u8; 32] = msg.merkle_root.as_slice().try_into().ok()?;
    let _seed: [u8; 32] = msg.seed.as_slice().try_into().ok()?;
    let mut chunk_indices = Vec::with_capacity(msg.chunk_indices.len());
    for index in &msg.chunk_indices {
        chunk_indices.push(usize::try_from(*index).ok()?);
    }
    Some(CapacityProofGenerationParams {
        challenge_id: ChallengeId::parse_hex(&msg.challenge_id).ok()?,
        challenger,
        provider_id: provider,
        chunk_indices,
        block_height: msg.block_height,
        expected_merkle_root: CapacityMerkleRoot::new(merkle_root),
        expiration_block: msg.expiration_block,
        timestamp: msg.timestamp,
    })
}

fn address_from(bytes: &[u8]) -> Option<Address> {
    let bytes: [u8; 20] = bytes.try_into().ok()?;
    Some(Address::from_bytes(bytes))
}

fn decode_hex_bytes(hex_text: &str, len: usize) -> Option<Vec<u8>> {
    let bytes = hex::decode(hex_text).ok()?;
    if bytes.len() == len {
        Some(bytes)
    } else {
        None
    }
}

pub(crate) fn blob_matches_content_id(content_id: &str, blob: &[u8]) -> bool {
    let digest = hex::encode(Sha256::digest(blob));
    let id = content_id
        .strip_prefix("0x")
        .or_else(|| content_id.strip_prefix("0X"))
        .unwrap_or(content_id);
    id.eq_ignore_ascii_case(&digest)
}
