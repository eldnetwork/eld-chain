//! Tendermint `summary` JSON-RPC. The result is the current summary header.

use eld_common::error::{EldError, ErrorBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Current chain summary header from the node `summary` method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryHeader {
    /// Height the summary covers through.
    pub end_height: String,
    /// Hash of the header.
    pub summary_hash: String,
    /// Block id at `end_height`.
    pub last_block_id: SummaryBlockId,
    /// Results hash at the frontier.
    pub last_results_hash: String,
    /// Application hash at the frontier.
    pub app_hash: String,
    /// Hash of the compressed snapshot body.
    pub body_hash: String,
    /// Hash of `validators`.
    pub validators_hash: String,
    /// Hash of `next_validators`.
    pub next_validators_hash: String,
    /// Hash of `consensus_params`.
    pub consensus_hash: String,
    /// Last height the validator set changed.
    pub last_height_validators_changed: String,
    /// Last height the consensus params changed.
    pub last_height_consensus_params_changed: String,
    /// Validator set at the frontier.
    pub validators: SummaryValidatorSet,
    /// Next validator set at the frontier.
    pub next_validators: SummaryValidatorSet,
    /// Consensus params at the frontier.
    pub consensus_params: SummaryConsensusParams,
}

/// Block id in a summary header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryBlockId {
    /// Block hash, uppercase hex.
    pub hash: String,
    /// Part set for that block.
    pub parts: SummaryPartSet,
}

/// Part-set header in a summary block id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryPartSet {
    /// Number of parts.
    pub total: u32,
    /// Part-set hash, uppercase hex.
    pub hash: String,
}

/// Validator set stored in the summary header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryValidatorSet {
    /// Validators in set order.
    pub validators: Vec<SummaryValidator>,
    /// Current proposer, when the set has one.
    pub proposer: Option<SummaryValidator>,
}

/// One validator in a summary validator set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryValidator {
    /// Address, uppercase hex.
    pub address: String,
    /// Amino public key, when present.
    pub pub_key: Option<Value>,
    /// Voting power.
    pub voting_power: i64,
    /// Proposer priority.
    pub proposer_priority: i64,
}

/// Consensus params stored in the summary header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryConsensusParams {
    /// Block limits.
    pub block: SummaryBlockParams,
    /// Evidence limits.
    pub evidence: SummaryEvidenceParams,
    /// Allowed public-key types.
    pub validator: SummaryValidatorParams,
    /// Application version.
    pub version: SummaryVersionParams,
}

/// Block params in a summary header. Integers are decimal strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryBlockParams {
    /// Maximum block size in bytes.
    pub max_bytes: String,
    /// Maximum block gas.
    pub max_gas: String,
    /// Minimum time between blocks, in milliseconds.
    pub time_iota_ms: String,
}

/// Evidence params in a summary header. Integers are decimal strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryEvidenceParams {
    /// Maximum age of evidence, in blocks.
    pub max_age_num_blocks: String,
    /// Maximum age of evidence, in nanoseconds.
    pub max_age_duration: String,
    /// Maximum evidence size in bytes.
    pub max_bytes: String,
}

/// Validator params in a summary header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryValidatorParams {
    /// Accepted public-key type names.
    pub pub_key_types: Vec<String>,
}

/// Version params in a summary header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryVersionParams {
    /// Application version.
    pub app_version: String,
}

/// Parse a JSON-RPC `summary` response body.
pub(crate) fn parse_summary_response(response: &str) -> Result<SummaryHeader, EldError> {
    let json: Value = serde_json::from_str(response).map_err(|err| EldError::NetworkError {
        operation: "summary".to_string(),
        details: format!("Failed to parse summary JSON: {err}"),
    })?;

    if let Some(err) = json.get("error").filter(|value| !value.is_null()) {
        let data = err.get("data").and_then(Value::as_str).unwrap_or("");
        if data == "summary not found" {
            return Err(ErrorBuilder::not_found_error("Summary", "current"));
        }
        return Err(EldError::NetworkError {
            operation: "summary".to_string(),
            details: format!("Tendermint summary RPC error: {err}"),
        });
    }

    let result = json.get("result").ok_or_else(|| EldError::NetworkError {
        operation: "summary".to_string(),
        details: "Tendermint summary response missing result".to_string(),
    })?;
    if result.is_null() {
        return Err(EldError::NetworkError {
            operation: "summary".to_string(),
            details: "Tendermint summary response has null result".to_string(),
        });
    }

    serde_json::from_value(result.clone()).map_err(|err| EldError::NetworkError {
        operation: "summary".to_string(),
        details: format!("Failed to parse summary header: {err}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_response() -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "end_height": "10",
                "summary_hash": "24ADB8A1F5355DF7A0CF13B47F080F87A894A097D586564D2771BE81DF4BF505",
                "last_block_id": {
                    "hash": "ABCD",
                    "parts": { "total": 1, "hash": "EF01" }
                },
                "last_results_hash": "11",
                "app_hash": "22",
                "body_hash": "A606345E926BC749571988564AA8D506FFB506D5B0E40A2BFD6B819594A74146",
                "validators_hash": "33",
                "next_validators_hash": "44",
                "consensus_hash": "55",
                "last_height_validators_changed": "1",
                "last_height_consensus_params_changed": "1",
                "validators": {
                    "validators": [{
                        "address": "8B81CC2BA41D4C29D0A0F90123396993491C3EA6",
                        "pub_key": {
                            "type": "tendermint/PubKeyEd25519",
                            "value": "7EtfyZfBMTZ+rh7zHE+swgcH6roeoRYOvdFMoDW9+eU="
                        },
                        "voting_power": 10,
                        "proposer_priority": -10
                    }],
                    "proposer": {
                        "address": "8B81CC2BA41D4C29D0A0F90123396993491C3EA6",
                        "pub_key": {
                            "type": "tendermint/PubKeyEd25519",
                            "value": "7EtfyZfBMTZ+rh7zHE+swgcH6roeoRYOvdFMoDW9+eU="
                        },
                        "voting_power": 10,
                        "proposer_priority": -10
                    }
                },
                "next_validators": {
                    "validators": [{
                        "address": "8B81CC2BA41D4C29D0A0F90123396993491C3EA6",
                        "pub_key": null,
                        "voting_power": 10,
                        "proposer_priority": 0
                    }],
                    "proposer": null
                },
                "consensus_params": {
                    "block": {
                        "max_bytes": "22020096",
                        "max_gas": "-1",
                        "time_iota_ms": "1000"
                    },
                    "evidence": {
                        "max_age_num_blocks": "100000",
                        "max_age_duration": "172800000000000",
                        "max_bytes": "1048576"
                    },
                    "validator": { "pub_key_types": ["ed25519"] },
                    "version": { "app_version": "0" }
                }
            }
        })
        .to_string()
    }

    #[test]
    fn parse_summary_reads_string_heights_and_validator_power() {
        let header = parse_summary_response(&sample_response()).unwrap();
        assert_eq!(header.end_height, "10");
        assert_eq!(
            header.summary_hash,
            "24ADB8A1F5355DF7A0CF13B47F080F87A894A097D586564D2771BE81DF4BF505"
        );
        assert_eq!(header.last_block_id.hash, "ABCD");
        assert_eq!(header.last_block_id.parts.total, 1);
        assert_eq!(header.validators.validators.len(), 1);
        assert_eq!(header.validators.validators[0].voting_power, 10);
        assert_eq!(
            header
                .validators
                .proposer
                .as_ref()
                .map(|v| v.address.as_str()),
            Some("8B81CC2BA41D4C29D0A0F90123396993491C3EA6")
        );
        assert!(header.next_validators.proposer.is_none());
        assert_eq!(header.consensus_params.version.app_version, "0");
        assert_eq!(header.consensus_params.block.max_gas, "-1");
    }

    #[test]
    fn parse_summary_maps_missing_header_to_not_found() {
        let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"Internal error","data":"summary not found"}}"#;
        let err = parse_summary_response(body).unwrap_err();
        assert_eq!(err.to_string(), "Summary not found: current");
    }
}
