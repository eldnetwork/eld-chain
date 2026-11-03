use super::info::{QueryProcessorResult, QueryResponsePayload};
use crate::app_state::FrontierSnapshotStore;
use abci::types::ResponseQuery;
use eld_common::constants::abci_query;
use prost::Message;

/// Name of the `/frontier_appstate` query token variable.
pub const SNAPSHOT_QUERY_TOKEN_KEY: &str = "ELD_SNAPSHOT_QUERY_TOKEN";

/// Handles a `/frontier_appstate` request that `EldAbciQuery` already selected.
///
/// Request data is `ELD_SNAPSHOT_QUERY_TOKEN` followed by the frontier as a
/// little-endian `i64`. A missing token, a mismatched token, or a frontier that
/// is not 8 bytes is code 403. An unknown frontier is code 0 with an empty
/// value. A hit puts a `SummaryBody` in `value`; `state_snapshot` is the stored
/// compressed bytes. `key` is the frontier. `height` is the current block height.
/// The token is not copied into the response or the log.
pub(crate) fn process_frontier_appstate_query(
    snapshots: &FrontierSnapshotStore,
    block_height: i64,
    _path: String,
    data: Vec<u8>,
) -> QueryProcessorResult {
    let response = frontier_appstate_query(snapshots, block_height, &data);
    Ok(QueryResponsePayload {
        code: response.code,
        value: response.value,
        log: response.log,
        info: response.info,
    })
}

fn frontier_appstate_query(
    snapshots: &FrontierSnapshotStore,
    block_height: i64,
    data: &[u8],
) -> ResponseQuery {
    let Some(token) = snapshot_query_token() else {
        return frontier_appstate_forbidden(block_height);
    };
    let Some(frontier) = frontier_from_request(data, &token) else {
        return frontier_appstate_forbidden(block_height);
    };
    let Some(compressed) = snapshots.get(frontier) else {
        return frontier_appstate_response(0, "", frontier, block_height, Vec::new());
    };
    let body = eld_tendermint_proto::types::SummaryBody {
        state_snapshot: compressed,
    };
    frontier_appstate_response(0, "", frontier, block_height, body.encode_to_vec())
}

fn snapshot_query_token() -> Option<String> {
    let token = std::env::var(SNAPSHOT_QUERY_TOKEN_KEY).ok()?;
    if token.is_empty() {
        return None;
    }
    Some(token)
}

/// Token bytes, then the frontier argument as 8 little-endian bytes.
fn frontier_from_request(data: &[u8], token: &str) -> Option<i64> {
    let frontier_bytes = data.strip_prefix(token.as_bytes())?;
    let frontier_bytes: [u8; 8] = frontier_bytes.try_into().ok()?;
    Some(i64::from_le_bytes(frontier_bytes))
}

fn frontier_appstate_forbidden(block_height: i64) -> ResponseQuery {
    ResponseQuery {
        code: 403,
        log: "snapshot query forbidden".to_string(),
        key: Vec::new(),
        value: Vec::new(),
        height: block_height,
        codespace: abci_query::FRONTIER_APPSTATE.to_string(),
        info: String::new(),
        index: 0,
        proof_ops: None,
    }
}

fn frontier_appstate_response(
    code: u32,
    log: &str,
    frontier: i64,
    block_height: i64,
    value: Vec<u8>,
) -> ResponseQuery {
    ResponseQuery {
        code,
        log: log.to_string(),
        key: frontier.to_le_bytes().to_vec(),
        value,
        height: block_height,
        codespace: abci_query::FRONTIER_APPSTATE.to_string(),
        info: String::new(),
        index: 0,
        proof_ops: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn with_snapshot_token<T>(token: Option<&str>, body: impl FnOnce() -> T) -> T {
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock().expect("snapshot token test lock");
        match token {
            Some(value) => std::env::set_var(SNAPSHOT_QUERY_TOKEN_KEY, value),
            None => std::env::remove_var(SNAPSHOT_QUERY_TOKEN_KEY),
        }
        let result = body();
        std::env::remove_var(SNAPSHOT_QUERY_TOKEN_KEY);
        result
    }

    fn frontier_request(token: &str, frontier: i64) -> Vec<u8> {
        let mut data = token.as_bytes().to_vec();
        data.extend_from_slice(&frontier.to_le_bytes());
        data
    }

    fn store_with_frontier(frontier: i64, compressed: Vec<u8>) -> FrontierSnapshotStore {
        let store = FrontierSnapshotStore::new();
        store.insert(frontier, compressed);
        store
    }

    #[test]
    fn frontier_appstate_query_returns_summary_body_for_matching_token() {
        let compressed = vec![1, 2, 3, 4];
        let store = store_with_frontier(4, compressed.clone());
        let response = with_snapshot_token(Some("correct-snapshot"), || {
            frontier_appstate_query(&store, 12, &frontier_request("correct-snapshot", 4))
        });
        assert_eq!(response.code, 0);
        assert_eq!(response.key, 4_i64.to_le_bytes().to_vec());
        assert_eq!(response.height, 12);
        assert!(response.log.is_empty());
        let body = eld_tendermint_proto::types::SummaryBody::decode(response.value.as_slice())
            .expect("summary body");
        assert_eq!(body.state_snapshot, compressed);
    }

    #[test]
    fn frontier_appstate_query_returns_empty_when_frontier_is_not_ready() {
        let store = FrontierSnapshotStore::new();
        let response = with_snapshot_token(Some("correct-snapshot"), || {
            frontier_appstate_query(&store, 12, &frontier_request("correct-snapshot", 4))
        });
        assert_eq!(response.code, 0);
        assert_eq!(response.key, 4_i64.to_le_bytes().to_vec());
        assert_eq!(response.height, 12);
        assert!(response.log.is_empty());
        assert!(response.value.is_empty());
    }

    #[test]
    fn frontier_appstate_query_rejects_wrong_token() {
        let store = store_with_frontier(4, vec![1]);
        let response = with_snapshot_token(Some("correct-snapshot"), || {
            frontier_appstate_query(&store, 12, &frontier_request("other-snapshot", 4))
        });
        assert_eq!(response.code, 403);
        assert_eq!(response.log, "snapshot query forbidden");
        assert_eq!(response.height, 12);
        assert!(response.value.is_empty());
        assert!(response.key.is_empty());
    }

    #[test]
    fn frontier_appstate_query_rejects_missing_token() {
        let store = store_with_frontier(4, vec![1]);
        let response = with_snapshot_token(None, || {
            frontier_appstate_query(&store, 12, &frontier_request("correct-snapshot", 4))
        });
        assert_eq!(response.code, 403);
        assert_eq!(response.log, "snapshot query forbidden");
        assert_eq!(response.height, 12);
        assert!(response.value.is_empty());
        assert!(response.key.is_empty());
    }

    #[test]
    fn frontier_appstate_query_rejects_token_without_frontier() {
        let store = store_with_frontier(4, vec![1]);
        let response = with_snapshot_token(Some("correct-snapshot"), || {
            frontier_appstate_query(&store, 12, b"correct-snapshot")
        });
        assert_eq!(response.code, 403);
        assert_eq!(response.log, "snapshot query forbidden");
        assert_eq!(response.height, 12);
        assert!(response.value.is_empty());
    }
}
