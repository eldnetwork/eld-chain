//! Tendermint `/tx` JSON-RPC parsing that accepts Go (string) and Rust (number) dialects.

use abci::types::{Event, EventAttribute};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use eld_common::error::EldError;
use serde_json::Value;

/// Committed transaction fields needed by the indexer from Tendermint `/tx`.
#[derive(Debug, Clone)]
pub struct TxByHashResult {
    pub height: u64,
    pub index: u32,
    pub tx: Vec<u8>,
    pub code: u32,
    pub gas_used: i64,
    pub events: Vec<Event>,
}

/// Parse a JSON-RPC `/tx` response body into [`TxByHashResult`].
///
/// Accepts Go Tendermint string-encoded int64 fields and eld-tendermint-rs numeric fields.
pub(crate) fn parse_tx_by_hash_response(response: &str) -> Result<TxByHashResult, EldError> {
    let json: Value = serde_json::from_str(response).map_err(|e| EldError::NetworkError {
        operation: "get_tx_by_hash".to_string(),
        details: format!("Failed to parse /tx JSON: {e}"),
    })?;

    if let Some(err) = json.get("error").filter(|e| !e.is_null()) {
        return Err(EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: format!("Tendermint /tx RPC error: {err}"),
        });
    }

    let result = json.get("result").ok_or_else(|| EldError::NetworkError {
        operation: "get_tx_by_hash".to_string(),
        details: "Tendermint /tx response missing result".to_string(),
    })?;

    if result.is_null() {
        return Err(EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: "Tendermint /tx response has null result".to_string(),
        });
    }

    parse_tx_by_hash_result(result)
}

pub(crate) fn parse_tx_by_hash_result(result: &Value) -> Result<TxByHashResult, EldError> {
    let height = rpc_json_u64(&result["height"], "height")?;
    let index = rpc_json_u64(&result["index"], "index")? as u32;

    let tx_b64 = result["tx"]
        .as_str()
        .ok_or_else(|| EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: format!("Missing or non-string tx in /tx result: {}", result["tx"]),
        })?;
    let tx = BASE64_STANDARD
        .decode(tx_b64)
        .map_err(|e| EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: format!("Failed to base64-decode tx: {e}"),
        })?;

    let tx_result = &result["tx_result"];
    let code = rpc_json_u64(&tx_result["code"], "tx_result.code")? as u32;
    let gas_used = rpc_json_i64(&tx_result["gas_used"], "tx_result.gas_used")?;
    let events = parse_tx_result_events(&tx_result["events"]);

    Ok(TxByHashResult {
        height,
        index,
        tx,
        code,
        gas_used,
        events,
    })
}

/// Accept JSON number or decimal string (Go Tendermint int64-as-string dialect).
pub(crate) fn rpc_json_u64(value: &Value, field: &str) -> Result<u64, EldError> {
    if let Some(n) = value.as_u64() {
        return Ok(n);
    }
    if let Some(n) = value.as_i64() {
        return u64::try_from(n).map_err(|_| EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: format!("Negative {field} in /tx response: {n}"),
        });
    }
    if let Some(s) = value.as_str() {
        return s.parse::<u64>().map_err(|e| EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: format!("Invalid string {field} in /tx response ({s}): {e}"),
        });
    }
    Err(EldError::NetworkError {
        operation: "get_tx_by_hash".to_string(),
        details: format!("Missing or non-numeric {field} in /tx response: {value}"),
    })
}

/// Accept JSON number or decimal string (Go Tendermint int64-as-string dialect).
pub(crate) fn rpc_json_i64(value: &Value, field: &str) -> Result<i64, EldError> {
    if let Some(n) = value.as_i64() {
        return Ok(n);
    }
    if let Some(n) = value.as_u64() {
        return i64::try_from(n).map_err(|_| EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: format!("{field} out of i64 range in /tx response: {n}"),
        });
    }
    if let Some(s) = value.as_str() {
        return s.parse::<i64>().map_err(|e| EldError::NetworkError {
            operation: "get_tx_by_hash".to_string(),
            details: format!("Invalid string {field} in /tx response ({s}): {e}"),
        });
    }
    // Missing gas_used defaults to 0 (protobuf default).
    if value.is_null() {
        return Ok(0);
    }
    Err(EldError::NetworkError {
        operation: "get_tx_by_hash".to_string(),
        details: format!("Missing or non-numeric {field} in /tx response: {value}"),
    })
}

fn parse_tx_result_events(events_value: &Value) -> Vec<Event> {
    let Some(events) = events_value.as_array() else {
        return Vec::new();
    };
    events
        .iter()
        .map(|event| {
            let r#type = event["type"].as_str().unwrap_or("").to_string();
            let attributes = event["attributes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|attr| parse_event_attribute(attr).ok())
                .collect();
            Event { r#type, attributes }
        })
        .collect()
}

fn parse_event_attribute(attr: &Value) -> Result<EventAttribute, EldError> {
    let key = decode_attr_bytes(&attr["key"], "event_attribute_key")?;
    let value = decode_attr_bytes(&attr["value"], "event_attribute_value")?;
    let index = attr["index"].as_bool().unwrap_or(false);
    Ok(EventAttribute { key, value, index })
}

/// TM 0.34 attributes are often base64; later dialects use plain UTF-8 strings.
fn decode_attr_bytes(value: &Value, field: &str) -> Result<Vec<u8>, EldError> {
    let s = value.as_str().ok_or_else(|| EldError::ValidationError {
        field: field.to_string(),
        value: value.to_string(),
        details: format!("{field} is not a string"),
    })?;
    if let Ok(decoded) = BASE64_STANDARD.decode(s) {
        return Ok(decoded);
    }
    Ok(s.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tx_b64() -> String {
        BASE64_STANDARD.encode(b"{\"nonce\":1}")
    }

    #[test]
    fn parse_go_string_height_and_gas() {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":1,"result":{{"hash":"AABB","height":"2","index":0,"tx_result":{{"code":0,"gas_wanted":"1","gas_used":"42","events":[]}},"tx":"{}"}}}}"#,
            sample_tx_b64()
        );
        let parsed = parse_tx_by_hash_response(&body).expect("parse");
        assert_eq!(parsed.height, 2);
        assert_eq!(parsed.index, 0);
        assert_eq!(parsed.code, 0);
        assert_eq!(parsed.gas_used, 42);
        assert_eq!(parsed.tx, b"{\"nonce\":1}");
    }

    #[test]
    fn parse_rs_numeric_height_and_gas() {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":1,"result":{{"hash":"AABB","height":2,"index":1,"tx_result":{{"code":0,"gas_wanted":1,"gas_used":7,"events":[]}},"tx":"{}"}}}}"#,
            sample_tx_b64()
        );
        let parsed = parse_tx_by_hash_response(&body).expect("parse");
        assert_eq!(parsed.height, 2);
        assert_eq!(parsed.index, 1);
        assert_eq!(parsed.gas_used, 7);
    }

    #[test]
    fn rpc_json_u64_rejects_garbage() {
        let err = rpc_json_u64(&Value::Bool(true), "height").unwrap_err();
        assert!(err.to_string().contains("height"));
    }
}
