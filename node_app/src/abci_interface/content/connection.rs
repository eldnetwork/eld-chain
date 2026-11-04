//! Content connection. Same level as the info and consensus connections.
//!
//! End users read chain records through the info connection's CADO query.
//! This connection answers a content-channel `ContentRequest` from the local slots.

use std::sync::Arc;

use abci::async_api::Content;
use abci::async_trait;
use abci::types::{RequestContent, ResponseContent};

use crate::capacity::capacity_manager::CapacityManager;

use super::handler;

pub struct ContentConnection {
    capacity: Arc<CapacityManager>,
}

impl ContentConnection {
    pub fn new(capacity: Arc<CapacityManager>) -> Self {
        Self { capacity }
    }
}

#[async_trait]
impl Content for ContentConnection {
    async fn content(&self, request: RequestContent) -> ResponseContent {
        response(&self.capacity, request).await
    }
}

pub async fn response(capacity: &Arc<CapacityManager>, request: RequestContent) -> ResponseContent {
    ResponseContent {
        message: handler::reply(capacity, &request.peer_id, &request.message).await,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom, Write};
    use std::sync::Arc;

    use abci::types::RequestContent;
    use eld_common::address::Address;
    use eld_common::capacity::{CapacityConfig, Slot, SlotMap};
    use eld_common::capacity_proof::SlotState;
    use eld_common::constants::pinboard::MAX_CHUNK_SIZE;
    use eld_tendermint_proto::content::{self, ContentPush, ContentRequest, Message};
    use prost::Message as ProstMessage;
    use tempfile::TempDir;

    use crate::capacity::capacity_manager::CapacityManager;

    use super::response;

    const PROVIDER: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn provider() -> Address {
        Address::parse_hex_str(PROVIDER).expect("provider")
    }

    fn manager(dir: &TempDir, wallet_dir: &TempDir) -> Arc<CapacityManager> {
        let wallet_path = wallet_dir.path().join("wallets.json");
        std::fs::write(&wallet_path, "[]").expect("wallets");
        let cli = Arc::new(
            eld_client::facade::ChainClient::with_wallets(
                eld_client::config::ClientConfig {
                    node_host: "127.0.0.1".to_string(),
                    node_port: "26657".to_string(),
                    chain_id: "test-chain".to_string(),
                    faucet_host: "127.0.0.1".to_string(),
                    faucet_port: "8080".to_string(),
                    faucet_end_point: "/faucet/request".to_string(),
                    faucet_url: None,
                    app_port: "9001".to_string(),
                    node_url: None,
                    app_url: None,
                },
                eld_common::fee::FeeConfig::default(),
                wallet_path,
            )
            .expect("client"),
        );
        Arc::new(CapacityManager::new(
            CapacityConfig {
                capacity_dir: dir.path().to_path_buf(),
                max_capacity_gb: 1,
                provider_id: provider(),
                auto_register: false,
                registration_retry_interval_secs: 60,
                tendermint_rpc_url: "http://127.0.0.1:26657".to_string(),
            },
            "wallet1".to_string(),
            cli,
            Arc::new(std::sync::Mutex::new(crate::config::ConsensusConfig {
                chain_id: "test-chain".to_string(),
                app_host: "127.0.0.1".to_string(),
                app_port: "26658".to_string(),
                accounts: std::collections::HashMap::new(),
                storage_limits: crate::config::StorageLimits::default(),
            })),
            eld_common::protocol_constants::ProtocolHandle::installed_local_dev(),
        ))
    }

    async fn manager_with_open_slot() -> (Arc<CapacityManager>, TempDir, TempDir) {
        let dir = TempDir::new().expect("capacity dir");
        let wallet_dir = TempDir::new().expect("wallet dir");
        let capacity = manager(&dir, &wallet_dir);
        let slot_map = SlotMap {
            slots: vec![Slot {
                offset: 0,
                size: MAX_CHUNK_SIZE,
                state: SlotState::Open,
            }],
            capacity_bytes: MAX_CHUNK_SIZE as u64,
            seed: [0x42; 32],
            provider_id: provider(),
        };
        let allocator = capacity.slot_allocator();
        let allocator = allocator.lock().await;
        let capacity_file = allocator.capacity_file_path().to_path_buf();
        let slots_file = capacity_file.with_extension("slots.json");
        drop(allocator);
        if let Some(parent) = capacity_file.parent() {
            std::fs::create_dir_all(parent).expect("capacity dir");
        }
        std::fs::write(
            &slots_file,
            serde_json::to_string_pretty(&slot_map).expect("slot map json"),
        )
        .expect("write slot map");
        let mut file = std::fs::File::create(&capacity_file).expect("capacity file");
        file.seek(SeekFrom::Start(0)).expect("seek");
        file.write_all(&vec![0u8; MAX_CHUNK_SIZE]).expect("zeros");
        drop(file);
        capacity.initialize().await.expect("initialize slots");
        (capacity, dir, wallet_dir)
    }

    fn request(sum: content::message::Sum) -> RequestContent {
        RequestContent {
            peer_id: "peer-1".to_owned(),
            message: Message { sum: Some(sum) }.encode_to_vec(),
        }
    }

    #[tokio::test]
    async fn a_push_returns_nothing() {
        let dir = TempDir::new().expect("dir");
        let wallet_dir = TempDir::new().expect("wallet dir");
        let capacity = manager(&dir, &wallet_dir);
        let reply = response(
            &capacity,
            request(content::message::Sum::ContentPush(ContentPush {
                request_id: 7,
                content_id: "id".to_owned(),
                blob: b"hello".to_vec(),
            })),
        )
        .await;
        assert!(reply.message.is_empty());
    }

    #[tokio::test]
    async fn a_request_returns_the_blob_from_the_slots() {
        let (capacity, _dir, _wallet_dir) = manager_with_open_slot().await;
        let content_id = "0xabc".to_owned();
        capacity
            .store_content_chunks(content_id.clone(), vec![b"hello".to_vec()])
            .await
            .expect("store");

        let reply = response(
            &capacity,
            request(content::message::Sum::ContentRequest(ContentRequest {
                request_id: 9,
                content_id: content_id.clone(),
            })),
        )
        .await;
        let message = Message::decode(reply.message.as_slice()).expect("reply");
        match message.sum {
            Some(content::message::Sum::ContentResponse(response)) => {
                assert_eq!(response.request_id, 9);
                assert_eq!(response.content_id, content_id);
                assert_eq!(response.blob, b"hello");
            }
            other => panic!("expected a content response, got {other:?}"),
        }

        let missing = response(
            &capacity,
            request(content::message::Sum::ContentRequest(ContentRequest {
                request_id: 10,
                content_id: "missing".to_owned(),
            })),
        )
        .await;
        assert!(missing.message.is_empty());
    }
}
