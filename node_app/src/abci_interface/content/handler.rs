//! Dispatch one content-channel message.
//!
//! The bytes are a `tendermint.content.Message`. An empty reply means nothing
//! is sent back. A content request is answered from the local slot store.

use std::sync::Arc;

use eld_tendermint_proto::content::{self, ContentResponse, Message};
use prost::Message as ProstMessage;
use tracing::error;

use crate::capacity::capacity_manager::CapacityManager;

pub async fn reply(
    capacity: &Arc<CapacityManager>,
    peer_id: &str,
    message_bytes: &[u8],
) -> Vec<u8> {
    let Ok(message) = Message::decode(message_bytes) else {
        return Vec::new();
    };
    let Some(sum) = message.sum else {
        return Vec::new();
    };
    let reply = match sum {
        content::message::Sum::ContentPush(msg) => handle_push(peer_id, &msg),
        content::message::Sum::ContentRequest(msg) => handle_request(capacity, &msg).await,
        content::message::Sum::CapacityChallenge(msg) => handle_challenge(peer_id, &msg),
        content::message::Sum::ContentPushAck(msg) => handle_push_ack(peer_id, &msg),
        content::message::Sum::ContentResponse(msg) => handle_response(peer_id, &msg),
        content::message::Sum::CapacityChallengeResponse(msg) => {
            handle_challenge_response(peer_id, &msg)
        }
        content::message::Sum::Announce(msg) => handle_announce(peer_id, &msg),
    };
    match reply {
        Some(message) => message.encode_to_vec(),
        None => Vec::new(),
    }
}

fn handle_push(_peer_id: &str, _msg: &content::ContentPush) -> Option<Message> {
    None
}

async fn handle_request(
    capacity: &CapacityManager,
    msg: &content::ContentRequest,
) -> Option<Message> {
    let blob = match capacity.get_content_from_slots(&msg.content_id).await {
        Ok(blob) => blob,
        Err(err) => {
            error!(
                content_id = %msg.content_id,
                error = %err,
                "content read failed"
            );
            return None;
        }
    };
    let blob = blob?;
    Some(Message {
        sum: Some(content::message::Sum::ContentResponse(ContentResponse {
            request_id: msg.request_id,
            content_id: msg.content_id.clone(),
            blob,
        })),
    })
}

fn handle_challenge(_peer_id: &str, _msg: &content::CapacityChallenge) -> Option<Message> {
    None
}

fn handle_push_ack(_peer_id: &str, _msg: &content::ContentPushAck) -> Option<Message> {
    None
}

fn handle_response(_peer_id: &str, _msg: &content::ContentResponse) -> Option<Message> {
    None
}

fn handle_challenge_response(
    _peer_id: &str,
    _msg: &content::CapacityChallengeResponse,
) -> Option<Message> {
    None
}

fn handle_announce(_peer_id: &str, _msg: &content::Announce) -> Option<Message> {
    None
}
