//! Content connection. Same level as the info and consensus connections.
//!
//! End users read chain records through the info connection's CADO query.
//! A content-channel message is answered here.

use std::sync::{Arc, Mutex, RwLock};

use abci::async_api::Content;
use abci::async_trait;
use abci::types::{RequestContent, ResponseContent};

use crate::app_state::AppState;
use crate::capacity::capacity_manager::CapacityManager;
use crate::capacity::missing_content_tracker::MissingContentTracker;
use crate::node_identity::LocalNodeIdentity;

use super::temp_storage::ContentTempStorage;

pub struct ContentParts<B> {
    pub capacity: Arc<CapacityManager>,
    pub blobs: Arc<B>,
    pub committed_state: Arc<Mutex<AppState>>,
    pub local_identity: Arc<RwLock<LocalNodeIdentity>>,
}

pub struct ContentConnection<B> {
    pub(super) capacity: Arc<CapacityManager>,
    pub(super) blobs: Arc<B>,
    pub(super) committed_state: Arc<Mutex<AppState>>,
    pub(super) local_identity: Arc<RwLock<LocalNodeIdentity>>,
    pub(super) missing: Mutex<MissingContentTracker>,
    pub(super) receipts: Mutex<Vec<ContentReceipt>>,
    pub(super) challenge_outcomes: Mutex<Vec<ChallengeOutcome>>,
}

pub struct ContentReceipt {
    pub request_id: u64,
    pub content_id: String,
    pub provider: Vec<u8>,
}

pub enum ChallengeOutcome {
    Verified {
        challenge_id: String,
        provider: Vec<u8>,
    },
    Rejected {
        challenge_id: String,
        provider: Vec<u8>,
        reason: String,
    },
}

impl<B> ContentConnection<B>
where
    B: ContentTempStorage,
{
    pub fn new(parts: ContentParts<B>) -> Self {
        let mut missing =
            MissingContentTracker::new(parts.capacity.config().capacity_dir.as_path());
        if let Err(err) = missing.load() {
            tracing::error!(error = %err, "missing content tracker did not load");
        }
        Self {
            capacity: parts.capacity,
            blobs: parts.blobs,
            committed_state: parts.committed_state,
            local_identity: parts.local_identity,
            missing: Mutex::new(missing),
            receipts: Mutex::new(Vec::new()),
            challenge_outcomes: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl<B> Content for ContentConnection<B>
where
    B: ContentTempStorage + 'static,
{
    async fn content(&self, request: RequestContent) -> ResponseContent {
        ResponseContent {
            message: self.answer(&request.message).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::{Seek, SeekFrom, Write};
    use std::sync::{Arc, Mutex, RwLock};

    use abci::async_api::Content;
    use abci::types::RequestContent;
    use eld_common::address::Address;
    use eld_common::capacity::{CapacityConfig, Slot, SlotMap};
    use eld_common::capacity_proof::SlotState;
    use eld_common::constants::pinboard::MAX_CHUNK_SIZE;
    use eld_common::error::EldError;
    use eld_tendermint_proto::content::{self, ContentPush, ContentRequest, Message};
    use prost::Message as ProstMessage;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use crate::app_state::AppState;
    use crate::capacity::capacity_manager::CapacityManager;
    use crate::node_identity::LocalNodeIdentity;

    use super::super::temp_storage::ContentTempStorage;
    use super::{ContentConnection, ContentParts};

    const PROVIDER: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    struct MemoryBlobs {
        blobs: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl ContentTempStorage for MemoryBlobs {
        fn read_temp_blob(&self, content_id: &str) -> Result<Option<Vec<u8>>, EldError> {
            Ok(self.blobs.lock().unwrap().get(content_id).cloned())
        }

        fn write_temp_blob(
            &self,
            content_id: &str,
            bytes: &[u8],
            _expires_at_unix: u64,
        ) -> Result<(), EldError> {
            self.blobs
                .lock()
                .unwrap()
                .insert(content_id.to_owned(), bytes.to_vec());
            Ok(())
        }
    }

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
            Arc::new(Mutex::new(crate::config::ConsensusConfig {
                chain_id: "test-chain".to_string(),
                app_host: "127.0.0.1".to_string(),
                app_port: "26658".to_string(),
                accounts: HashMap::new(),
                storage_limits: crate::config::StorageLimits::default(),
            })),
            eld_common::protocol_constants::ProtocolHandle::installed_local_dev(),
        ))
    }

    fn connection(capacity: Arc<CapacityManager>) -> ContentConnection<MemoryBlobs> {
        ContentConnection::new(ContentParts {
            capacity,
            blobs: Arc::new(MemoryBlobs {
                blobs: Mutex::new(HashMap::new()),
            }),
            committed_state: Arc::new(Mutex::new(AppState::default())),
            local_identity: Arc::new(RwLock::new(LocalNodeIdentity::default())),
        })
    }

    fn request(sum: content::message::Sum) -> RequestContent {
        RequestContent {
            peer_id: "peer-1".to_owned(),
            message: Message { sum: Some(sum) }.encode_to_vec(),
        }
    }

    fn hash_id(blob: &[u8]) -> String {
        hex::encode(Sha256::digest(blob))
    }

    #[tokio::test]
    async fn a_push_with_the_wrong_id_returns_nothing() {
        let dir = TempDir::new().expect("dir");
        let wallet_dir = TempDir::new().expect("wallet dir");
        let conn = connection(manager(&dir, &wallet_dir));
        let reply = conn
            .content(request(content::message::Sum::ContentPush(ContentPush {
                request_id: 7,
                content_id: "id".to_owned(),
                blob: b"hello".to_vec(),
            })))
            .await;
        assert!(reply.message.is_empty());
    }

    #[tokio::test]
    async fn a_push_without_slots_stores_a_temp_blob_and_returns_a_receipt() {
        let dir = TempDir::new().expect("dir");
        let wallet_dir = TempDir::new().expect("wallet dir");
        let conn = connection(manager(&dir, &wallet_dir));
        let blob = b"hello".to_vec();
        let content_id = hash_id(&blob);
        let reply = conn
            .content(request(content::message::Sum::ContentPush(ContentPush {
                request_id: 7,
                content_id: content_id.clone(),
                blob: blob.clone(),
            })))
            .await;
        let message = Message::decode(reply.message.as_slice()).expect("reply");
        match message.sum {
            Some(content::message::Sum::ContentPushAck(ack)) => {
                assert_eq!(ack.request_id, 7);
                assert_eq!(ack.content_id, content_id);
                assert_eq!(ack.provider, provider().as_bytes());
            }
            other => panic!("expected a receipt, got {other:?}"),
        }
        let fetched = conn
            .content(request(content::message::Sum::ContentRequest(
                ContentRequest {
                    request_id: 8,
                    content_id: content_id.clone(),
                },
            )))
            .await;
        let message = Message::decode(fetched.message.as_slice()).expect("blob");
        match message.sum {
            Some(content::message::Sum::ContentResponse(response)) => {
                assert_eq!(response.blob, blob);
            }
            other => panic!("expected the temp blob, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_request_returns_the_blob_from_the_slots() {
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

        let content_id = "0xabc".to_owned();
        capacity
            .store_content_chunks(content_id.clone(), vec![b"hello".to_vec()])
            .await
            .expect("store");
        let conn = connection(capacity);
        let reply = conn
            .content(request(content::message::Sum::ContentRequest(
                ContentRequest {
                    request_id: 9,
                    content_id: content_id.clone(),
                },
            )))
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
    }
}
