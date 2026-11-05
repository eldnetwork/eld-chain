use crate::capacity_proof_summary::{
    CapacityProofOutcome, CapacityProofSignature, CapacityProofSummary,
};
use crate::error::EldError;
use crate::tx::parts::HasSender;
use crate::Address;
use serde::{Deserialize, Serialize};

/// Transaction payload for one epoch's capacity-proof summary.
///
/// `sender` is the capacity validator who signed the summary.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
#[serde(try_from = "CapacityProofSummaryTxUnchecked")]
pub struct CapacityProofSummaryTx {
    pub sender: Address,
    pub epoch: i64,
    pub outcomes: Vec<CapacityProofOutcome>,
    pub signature: CapacityProofSignature,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapacityProofSummaryTxUnchecked {
    sender: Address,
    epoch: i64,
    outcomes: Vec<CapacityProofOutcome>,
    signature: CapacityProofSignature,
}

impl TryFrom<CapacityProofSummaryTxUnchecked> for CapacityProofSummaryTx {
    type Error = EldError;

    fn try_from(unchecked: CapacityProofSummaryTxUnchecked) -> Result<Self, Self::Error> {
        Ok(Self {
            sender: unchecked.sender,
            epoch: unchecked.epoch,
            outcomes: unchecked.outcomes,
            signature: unchecked.signature,
        })
    }
}

impl CapacityProofSummaryTx {
    /// The validator on `summary` is the transaction sender.
    #[must_use]
    pub fn from_summary(summary: CapacityProofSummary) -> Self {
        Self {
            sender: summary.validator,
            epoch: summary.epoch,
            outcomes: summary.outcomes,
            signature: summary.signature,
        }
    }

    /// Domain summary signed by [`Self::sender`].
    #[must_use]
    pub fn summary(&self) -> CapacityProofSummary {
        CapacityProofSummary {
            epoch: self.epoch,
            validator: self.sender,
            outcomes: self.outcomes.clone(),
            signature: self.signature,
        }
    }
}

impl std::fmt::Display for CapacityProofSummaryTx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CapacityProofSummaryTx {{\n sender: {}\n epoch: {}\n }}",
            self.sender, self.epoch
        )
    }
}

impl HasSender for CapacityProofSummaryTx {
    fn sender(&self) -> Address {
        self.sender
    }
}
