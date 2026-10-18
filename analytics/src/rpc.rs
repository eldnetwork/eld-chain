use serde::Deserialize;

/// Tip fields from Tendermint `status`, matching the explorer's
/// `result.sync_info.latest_block_height` read (see eld-blockchain-explorer
/// `useLatestHeight` / `fetchRpcStatus`).
#[derive(Debug, Deserialize)]
struct StatusResponse {
    result: Option<StatusResult>,
}

#[derive(Debug, Deserialize)]
struct StatusResult {
    sync_info: SyncInfo,
}

#[derive(Debug, Deserialize)]
struct SyncInfo {
    latest_block_height: String,
}

/// Fetch the chain tip height from Tendermint RPC.
///
/// Prefer URI `GET /status` (classic Tendermint / explorer). Fall back to
/// JSON-RPC `POST` `status` for eld-tendermint-rs, which returns 404 on GET.
pub async fn fetch_latest_block_height(rpc_url: &str) -> anyhow::Result<u64> {
    let base = rpc_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client.get(format!("{base}/status")).send().await {
        Ok(response) if response.status().is_success() => {
            let body: StatusResponse = response.json().await?;
            return parse_height(body);
        }
        Ok(response) => {
            tracing::debug!(
                status = %response.status(),
                "GET /status not usable; trying JSON-RPC POST"
            );
        }
        Err(e) => {
            tracing::debug!(error = %e, "GET /status failed; trying JSON-RPC POST");
        }
    }

    let body: StatusResponse = client
        .post(base)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "status",
            "params": []
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_height(body)
}

/// `num_unconfirmed_txs`: transaction count and total bytes still in the mempool.
///
/// Same GET-then-POST fallback as [`fetch_latest_block_height`].
pub async fn fetch_mempool_totals(
    client: &reqwest::Client,
    rpc_url: &str,
) -> anyhow::Result<(u64, u64)> {
    let base = rpc_url.trim_end_matches('/');
    let path = format!("{base}/num_unconfirmed_txs");

    match client.get(&path).send().await {
        Ok(response) if response.status().is_success() => {
            let body: serde_json::Value = response.json().await?;
            return parse_mempool_totals(&body);
        }
        Ok(response) => {
            tracing::debug!(
                status = %response.status(),
                "GET /num_unconfirmed_txs not usable; trying JSON-RPC POST"
            );
        }
        Err(e) => {
            tracing::debug!(error = %e, "GET /num_unconfirmed_txs failed; trying JSON-RPC POST");
        }
    }

    let body: serde_json::Value = client
        .post(base)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "num_unconfirmed_txs",
            "params": []
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_mempool_totals(&body)
}

pub(crate) fn parse_mempool_totals(body: &serde_json::Value) -> anyhow::Result<(u64, u64)> {
    let result = body
        .get("result")
        .ok_or_else(|| anyhow::anyhow!("num_unconfirmed_txs response missing result"))?;
    let txs = result
        .get("total")
        .or_else(|| result.get("n_txs"))
        .ok_or_else(|| anyhow::anyhow!("num_unconfirmed_txs missing total"))?;
    let bytes = result
        .get("total_bytes")
        .ok_or_else(|| anyhow::anyhow!("num_unconfirmed_txs missing total_bytes"))?;
    Ok((json_u64_field(txs)?, json_u64_field(bytes)?))
}

fn json_u64_field(value: &serde_json::Value) -> anyhow::Result<u64> {
    if let Some(n) = value.as_u64() {
        return Ok(n);
    }
    if let Some(raw) = value.as_str() {
        return raw
            .parse::<u64>()
            .map_err(|e| anyhow::anyhow!("invalid mempool number {raw:?}: {e}"));
    }
    anyhow::bail!("mempool field is not a number")
}

fn parse_height(body: StatusResponse) -> anyhow::Result<u64> {
    let raw = body
        .result
        .ok_or_else(|| anyhow::anyhow!("status response missing result"))?
        .sync_info
        .latest_block_height;
    raw.parse::<u64>()
        .map_err(|e| anyhow::anyhow!("invalid latest_block_height {raw:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_height_from_explorer_shaped_json() {
        let body: StatusResponse = serde_json::from_str(
            r#"{
              "jsonrpc": "2.0",
              "id": 1,
              "result": {
                "sync_info": {
                  "latest_block_height": "7876"
                }
              }
            }"#,
        )
        .unwrap();
        assert_eq!(parse_height(body).unwrap(), 7876);
    }

    #[test]
    fn parse_mempool_accepts_string_totals() {
        let body = serde_json::json!({
            "result": { "total": "3", "total_bytes": "1200" }
        });
        assert_eq!(parse_mempool_totals(&body).unwrap(), (3, 1200));
    }

    #[test]
    fn parse_mempool_accepts_n_txs_alias() {
        let body = serde_json::json!({
            "result": { "n_txs": 4, "total_bytes": 50 }
        });
        assert_eq!(parse_mempool_totals(&body).unwrap(), (4, 50));
    }
}
