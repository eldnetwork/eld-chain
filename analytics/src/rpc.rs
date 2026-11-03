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
}
