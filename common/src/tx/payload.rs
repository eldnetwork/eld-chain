#![allow(missing_docs)]

use crate::tx::parts::{HasAmount, HasSender};
use crate::tx::payloads::{
    AddNamespaceTx, CapacityProofSummaryTx, CapacityProofTx, PostMessageTx, RegisterCapacityTx,
    StakeTx, TransferTx, UnregisterCapacityTx, UnstakeTx,     UpdateCapacityMerkleRootTx,
};
use crate::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Payload {
    pub r#type: String, // Renamed to "type" with serde rename
    #[serde(flatten)]
    pub inner: PayloadInner,
}

impl std::fmt::Display for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Payload {{\n type: {}\n inner: {}\n }}",
            self.r#type, self.inner
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxType {
    Transfer,
    Unstake,
    Stake,
    CapacityProof,
    RegisterCapacity,
    UnregisterCapacity,
    UpdateCapacityMerkleRoot,
    PostMessage,
    AddNamespace,
    CapacityProofSummary,
}

impl TxType {
    pub fn as_str(self) -> &'static str {
        match self {
            TxType::Transfer => crate::constants::tx_type::TX_TYPE_TRANSFER,
            TxType::Unstake => crate::constants::tx_type::TX_TYPE_UNSTAKE,
            TxType::Stake => crate::constants::tx_type::TX_TYPE_STAKE,
            TxType::CapacityProof => crate::constants::tx_type::TX_TYPE_CAPACITY_PROOF,
            TxType::RegisterCapacity => crate::constants::tx_type::TX_TYPE_REGISTER_CAPACITY,
            TxType::UnregisterCapacity => crate::constants::tx_type::TX_TYPE_UNREGISTER_CAPACITY,
            TxType::UpdateCapacityMerkleRoot => {
                crate::constants::tx_type::TX_TYPE_UPDATE_CAPACITY_MERKLE_ROOT
            }
            TxType::PostMessage => crate::constants::tx_type::TX_TYPE_POST_MESSAGE,
            TxType::AddNamespace => crate::constants::tx_type::TX_TYPE_ADD_NAMESPACE,
            TxType::CapacityProofSummary => {
                crate::constants::tx_type::TX_TYPE_CAPACITY_PROOF_SUMMARY
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(untagged)] // No extra "type" tag in JSON
pub enum PayloadInner {
    Transfer(TransferTx),
    Unstake(UnstakeTx), // needs to fo before stake
    Stake(StakeTx),
    CapacityProof(CapacityProofTx), // Signed challenge response, checked when delivered
    RegisterCapacity(RegisterCapacityTx), // Capacity proof registration
    UnregisterCapacity(UnregisterCapacityTx), // Capacity proof unregistration
    UpdateCapacityMerkleRoot(UpdateCapacityMerkleRootTx), // Update merkle root after content storage
    PostMessage(PostMessageTx),
    AddNamespace(AddNamespaceTx),
    CapacityProofSummary(CapacityProofSummaryTx),
}

impl From<TransferTx> for PayloadInner {
    fn from(tx: TransferTx) -> Self {
        PayloadInner::Transfer(tx)
    }
}

impl From<UnstakeTx> for PayloadInner {
    fn from(tx: UnstakeTx) -> Self {
        PayloadInner::Unstake(tx)
    }
}

impl From<StakeTx> for PayloadInner {
    fn from(tx: StakeTx) -> Self {
        PayloadInner::Stake(tx)
    }
}

impl From<CapacityProofTx> for PayloadInner {
    fn from(tx: CapacityProofTx) -> Self {
        PayloadInner::CapacityProof(tx)
    }
}

impl From<RegisterCapacityTx> for PayloadInner {
    fn from(tx: RegisterCapacityTx) -> Self {
        PayloadInner::RegisterCapacity(tx)
    }
}

impl From<UnregisterCapacityTx> for PayloadInner {
    fn from(tx: UnregisterCapacityTx) -> Self {
        PayloadInner::UnregisterCapacity(tx)
    }
}

impl From<UpdateCapacityMerkleRootTx> for PayloadInner {
    fn from(tx: UpdateCapacityMerkleRootTx) -> Self {
        PayloadInner::UpdateCapacityMerkleRoot(tx)
    }
}

impl From<PostMessageTx> for PayloadInner {
    fn from(tx: PostMessageTx) -> Self {
        PayloadInner::PostMessage(tx)
    }
}

impl From<AddNamespaceTx> for PayloadInner {
    fn from(tx: AddNamespaceTx) -> Self {
        PayloadInner::AddNamespace(tx)
    }
}

impl From<CapacityProofSummaryTx> for PayloadInner {
    fn from(tx: CapacityProofSummaryTx) -> Self {
        PayloadInner::CapacityProofSummary(tx)
    }
}

impl PayloadInner {
    pub fn tx_type(&self) -> TxType {
        match self {
            PayloadInner::Transfer(_) => TxType::Transfer,
            PayloadInner::Unstake(_) => TxType::Unstake,
            PayloadInner::Stake(_) => TxType::Stake,
            PayloadInner::CapacityProof(_) => TxType::CapacityProof,
            PayloadInner::RegisterCapacity(_) => TxType::RegisterCapacity,
            PayloadInner::UnregisterCapacity(_) => TxType::UnregisterCapacity,
            PayloadInner::UpdateCapacityMerkleRoot(_) => TxType::UpdateCapacityMerkleRoot,
            PayloadInner::PostMessage(_) => TxType::PostMessage,
            PayloadInner::AddNamespace(_) => TxType::AddNamespace,
            PayloadInner::CapacityProofSummary(_) => TxType::CapacityProofSummary,
        }
    }
}

impl Payload {
    pub fn new<T>(payload_tx: T) -> Self
    where
        T: Into<PayloadInner>,
    {
        let inner = payload_tx.into();
        Self {
            r#type: inner.tx_type().as_str().to_string(),
            inner,
        }
    }
}

impl std::fmt::Display for PayloadInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayloadInner::Transfer(tx) => write!(f, "Transfer({tx})"),
            PayloadInner::Unstake(tx) => write!(f, "Unstake({tx})"),
            PayloadInner::Stake(tx) => write!(f, "Stake({tx})"),
            PayloadInner::CapacityProof(tx) => write!(f, "CapacityProof({tx})"),
            PayloadInner::RegisterCapacity(tx) => write!(f, "RegisterCapacity({tx})"),
            PayloadInner::UnregisterCapacity(tx) => write!(f, "UnregisterCapacity({tx})"),
            PayloadInner::UpdateCapacityMerkleRoot(tx) => {
                write!(f, "UpdateCapacityMerkleRoot({tx})")
            }
            PayloadInner::PostMessage(tx) => write!(f, "PostMessage({tx})"),
            PayloadInner::AddNamespace(tx) => write!(f, "AddNamespace({tx})"),
            PayloadInner::CapacityProofSummary(tx) => write!(f, "CapacityProofSummary({tx})"),
        }
    }
}

impl HasSender for PayloadInner {
    fn sender(&self) -> Address {
        match self {
            PayloadInner::Transfer(tx) => tx.sender(),
            PayloadInner::Stake(tx) => tx.sender(),
            PayloadInner::Unstake(tx) => tx.sender(),
            PayloadInner::RegisterCapacity(tx) => tx.sender(),
            PayloadInner::UnregisterCapacity(tx) => tx.sender(),
            PayloadInner::UpdateCapacityMerkleRoot(tx) => tx.sender(),
            PayloadInner::PostMessage(tx) => tx.sender(),
            PayloadInner::CapacityProof(tx) => tx.sender(),
            PayloadInner::AddNamespace(tx) => tx.sender(),
            PayloadInner::CapacityProofSummary(tx) => tx.sender(),
        }
    }
}

impl HasAmount for PayloadInner {
    fn amount(&self) -> u128 {
        match self {
            PayloadInner::Transfer(tx) => tx.amount(),
            PayloadInner::Stake(tx) => tx.amount(),
            PayloadInner::Unstake(tx) => tx.amount(),
            PayloadInner::RegisterCapacity(tx) => tx.amount(),
            PayloadInner::UnregisterCapacity(tx) => tx.amount(),
            PayloadInner::UpdateCapacityMerkleRoot(tx) => tx.amount(),
            PayloadInner::PostMessage(tx) => tx.amount(),
            PayloadInner::CapacityProof(_) => 0,
            PayloadInner::AddNamespace(tx) => tx.amount(),
            PayloadInner::CapacityProofSummary(_) => 0,
        }
    }
}
