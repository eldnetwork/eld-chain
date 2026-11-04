//! Pinboard upload and fetch over the Tendermint content channel.
//!
//! The app asks the local node, on the RPC port it already uses for
//! `broadcast_tx_sync`, to queue a push or a request. The node sends that on
//! channel `0x50`. The receipt or blob comes back through the content connection.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use eld_common::Address;
use serde_json::Value;

use crate::abci_interface::content::blob_matches_content_id;
use crate::app_state::AppState;
use crate::capacity::capacity_manager::CapacityManager;

use super::error::ApiError;

const CONTENT_RPC_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_PEERS: usize = 3;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn tendermint_rpc_url() -> String {
    std::env::var("TENDERMINT_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:26657".to_string())
}

/// Capacity providers other than this node, at most three.
pub(crate) fn providers_to_push(state: &AppState, local: &Address) -> Vec<Address> {
    state
        .envelope
        .capacity_validators
        .iter()
        .map(|provider| provider.address)
        .filter(|address| address != local)
        .take(MAX_PEERS)
        .collect()
}

/// Push `blob` to connected peers. Returns once a selected provider has acked.
///
/// No other providers means this node is the only store, so the post can go out.
pub(crate) async fn push_to_providers(
    providers: &[Address],
    content_id: &str,
    blob: &[u8],
) -> Result<(), ApiError> {
    if providers.is_empty() {
        return Ok(());
    }
    let url = tendermint_rpc_url();
    let peers = connected_peers(&url).await?;
    if peers.is_empty() {
        return Err(ApiError::ServiceUnavailable {
            message: "no_content_peers".to_string(),
            details: Some("No Tendermint peer is connected to receive the blob".to_string()),
        });
    }
    let mut acks = 0usize;
    for peer_id in peers.into_iter().take(MAX_PEERS) {
        let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let Ok(provider) = push_one(&url, &peer_id, request_id, content_id, blob).await else {
            continue;
        };
        if providers
            .iter()
            .any(|selected| provider.as_slice() == selected.as_bytes())
        {
            acks += 1;
        }
    }
    if acks == 0 {
        return Err(ApiError::ServiceUnavailable {
            message: "content_push_unacked".to_string(),
            details: Some("No selected capacity provider accepted the blob".to_string()),
        });
    }
    Ok(())
}

/// Slots, then the temp blob, then one fetch from a connected peer.
pub(crate) async fn load_content(
    capacity: &CapacityManager,
    storage: &crate::storage::rocksdb::RocksDBStorage,
    content_id: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    if let Some(blob) = capacity
        .get_content_from_slots(content_id)
        .await
        .map_err(|err| ApiError::InternalServerError {
            message: "content_slot_read_failed".to_string(),
            details: Some(err.to_string()),
        })?
    {
        if blob_matches_content_id(content_id, &blob) {
            return Ok(Some(blob));
        }
    }
    if let Some(blob) =
        storage
            .get_pinboard_temp_blob(content_id)
            .map_err(|err| ApiError::InternalServerError {
                message: "content_temp_read_failed".to_string(),
                details: Some(err.to_string()),
            })?
    {
        if blob_matches_content_id(content_id, &blob) {
            return Ok(Some(blob));
        }
    }
    let url = tendermint_rpc_url();
    let Ok(peers) = connected_peers(&url).await else {
        return Ok(None);
    };
    for peer_id in peers.into_iter().take(MAX_PEERS) {
        let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let Ok(blob) = request_one(&url, &peer_id, request_id, content_id).await else {
            continue;
        };
        if blob_matches_content_id(content_id, &blob) {
            return Ok(Some(blob));
        }
    }
    Ok(None)
}

async fn push_one(
    url: &str,
    peer_id: &str,
    request_id: u64,
    content_id: &str,
    blob: &[u8],
) -> Result<Vec<u8>, ApiError> {
    let result = rpc(
        url,
        "content_push",
        serde_json::json!({
            "peer_id": peer_id,
            "request_id": request_id,
            "content_id": content_id,
            "blob": BASE64_STANDARD.encode(blob),
        }),
    )
    .await?;
    let provider = result
        .get("provider")
        .and_then(Value::as_str)
        .and_then(|text| BASE64_STANDARD.decode(text).ok())
        .ok_or_else(|| ApiError::InternalServerError {
            message: "content_push_receipt".to_string(),
            details: Some("receipt did not include a provider".to_string()),
        })?;
    Ok(provider)
}

async fn request_one(
    url: &str,
    peer_id: &str,
    request_id: u64,
    content_id: &str,
) -> Result<Vec<u8>, ApiError> {
    let result = rpc(
        url,
        "content_request",
        serde_json::json!({
            "peer_id": peer_id,
            "request_id": request_id,
            "content_id": content_id,
        }),
    )
    .await?;
    result
        .get("blob")
        .and_then(Value::as_str)
        .and_then(|text| BASE64_STANDARD.decode(text).ok())
        .ok_or_else(|| ApiError::InternalServerError {
            message: "content_request_blob".to_string(),
            details: Some("content response did not include a blob".to_string()),
        })
}

async fn connected_peers(url: &str) -> Result<Vec<String>, ApiError> {
    let result = rpc(url, "net_info", Value::Null).await?;
    let Some(peers) = result.get("peers").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    Ok(peers
        .iter()
        .filter_map(|peer| {
            peer.get("node_info")
                .and_then(|info| info.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect())
}

async fn rpc(url: &str, method: &str, params: Value) -> Result<Value, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(CONTENT_RPC_TIMEOUT)
        .build()
        .map_err(|err| ApiError::InternalServerError {
            message: "content_rpc_client".to_string(),
            details: Some(err.to_string()),
        })?;
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }))
        .send()
        .await
        .map_err(|err| ApiError::ServiceUnavailable {
            message: "content_rpc_unreachable".to_string(),
            details: Some(err.to_string()),
        })?;
    let body: Value = response
        .json()
        .await
        .map_err(|err| ApiError::InternalServerError {
            message: "content_rpc_response".to_string(),
            details: Some(err.to_string()),
        })?;
    if let Some(error) = body.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("content rpc failed");
        return Err(ApiError::ServiceUnavailable {
            message: "content_rpc_failed".to_string(),
            details: Some(message.to_string()),
        });
    }
    body.get("result")
        .cloned()
        .ok_or_else(|| ApiError::InternalServerError {
            message: "content_rpc_result".to_string(),
            details: Some("Tendermint content RPC returned no result".to_string()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use eld_common::coin::Coin;
    use eld_common::public_key::PublicKey;
    use eld_common::validator::CapacityValidatorInfo;
    use sha2::{Digest, Sha256};

    fn provider(byte: u8) -> CapacityValidatorInfo {
        CapacityValidatorInfo {
            address: Address::from_bytes([byte; 20]),
            stake: Coin::zero(),
            public_key: PublicKey::from(SigningKey::from_bytes(&[byte; 32]).verifying_key()),
            storage_capacity: 1,
            merkle_root: None,
            seed: None,
            chunk_count: None,
            registered_at: None,
            last_merkle_root_update: None,
            registered_block: 0,
            registration_duration: 0,
        }
    }

    #[test]
    fn providers_to_push_skips_this_node_and_keeps_three() {
        let local = Address::from_bytes([1; 20]);
        let mut state = AppState::default();
        state.envelope.capacity_validators = (1..6).map(provider).collect();
        let selected = providers_to_push(&state, &local);
        assert_eq!(selected.len(), 3);
        assert!(selected.iter().all(|address| address != &local));
    }

    #[test]
    fn a_blob_matches_its_content_id() {
        let blob = b"hello";
        let id = hex::encode(Sha256::digest(blob));
        assert!(blob_matches_content_id(&id, blob));
        assert!(!blob_matches_content_id("id", blob));
    }
}
