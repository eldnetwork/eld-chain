//! Content-channel encoding of a capacity chunk proof.

use eld_common::capacity_proof::{ChunkProof, SlotState};
use eld_tendermint_proto::content::{
    self, ChunkProof as ProtoChunkProof, SlotState as ProtoSlotState,
};

pub(super) fn chunk_proof_to_proto(proof: &ChunkProof) -> Option<ProtoChunkProof> {
    Some(ProtoChunkProof {
        chunk_index: u64::try_from(proof.chunk_index).ok()?,
        chunk_data: proof.chunk_data.clone(),
        chunk_hash: proof.chunk_hash.to_vec(),
        merkle_proof: proof
            .merkle_proof
            .iter()
            .map(|step| step.to_vec())
            .collect(),
        slot_state: Some(slot_state_to_proto(&proof.slot_state)),
    })
}

pub(super) fn chunk_proof_from_proto(proof: &ProtoChunkProof) -> Option<ChunkProof> {
    let chunk_hash: [u8; 32] = proof.chunk_hash.as_slice().try_into().ok()?;
    let mut merkle_proof = Vec::with_capacity(proof.merkle_proof.len());
    for step in &proof.merkle_proof {
        merkle_proof.push(step.as_slice().try_into().ok()?);
    }
    Some(ChunkProof {
        chunk_index: usize::try_from(proof.chunk_index).ok()?,
        chunk_data: proof.chunk_data.clone(),
        chunk_hash,
        merkle_proof,
        slot_state: slot_state_from_proto(proof.slot_state.as_ref()?)?,
    })
}

fn slot_state_to_proto(state: &SlotState) -> ProtoSlotState {
    let kind = match state {
        SlotState::Proof => content::slot_state::Kind::Proof(content::ProofSlot {}),
        SlotState::Open => content::slot_state::Kind::Open(content::OpenSlot {}),
        SlotState::Content {
            deal_id,
            committed_hash,
        } => content::slot_state::Kind::Content(content::ContentSlot {
            deal_id: deal_id.clone(),
            committed_hash: committed_hash.to_vec(),
        }),
    };
    ProtoSlotState { kind: Some(kind) }
}

fn slot_state_from_proto(state: &ProtoSlotState) -> Option<SlotState> {
    match state.kind.as_ref()? {
        content::slot_state::Kind::Proof(_) => Some(SlotState::Proof),
        content::slot_state::Kind::Open(_) => Some(SlotState::Open),
        content::slot_state::Kind::Content(slot) => {
            let committed_hash: [u8; 32] = slot.committed_hash.as_slice().try_into().ok()?;
            Some(SlotState::Content {
                deal_id: slot.deal_id.clone(),
                committed_hash,
            })
        }
    }
}
