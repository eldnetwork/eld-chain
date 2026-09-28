//! Human-facing CLI text. Callers print the returned strings on stdout.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use eld_client::api::abci::deliver_tx_events;
use eld_client::api::rest::{NamespaceRegisteredResponse, PostMessageSubmitResponse};
use eld_client::facade::ChainClient;
use eld_client::facade::NamespaceLookup;
use eld_client::facade::SubmittedTx;
use eld_client::logging::{SanitizedLog, SanitizedLoggable};
use eld_common::account::Account;
use eld_common::constants::cado::{
    PATH_PREFIX_ACCOUNT, PATH_PREFIX_ACCOUNT_CONTENT, PATH_PREFIX_APP_STATE_SNAPSHOT,
    PATH_PREFIX_CADO_MAP, PATH_PREFIX_STAKING_ACCOUNT,
};
use eld_common::error::{EldError, ErrorBuilder};
use eld_common::staking_account::StakingAccount;
use eld_common::validator::{ActiveValidatorsInfo, EpochInfo};
use eld_common::wallet::Wallet;
use serde_json::Value;

struct Text {
    lines: Vec<String>,
}

impl Text {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn line(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    fn blank(&mut self) {
        self.lines.push(String::new());
    }

    fn finish(self) -> String {
        self.lines.join("\n")
    }
}

pub(crate) fn print_result(text: &str) {
    let text = text.trim_end_matches('\n');
    if text.is_empty() {
        return;
    }
    println!("{text}");
}

pub(crate) fn created_wallet(wallet: &Wallet) -> String {
    wallet.terminal_display()
}

pub(crate) fn list_wallets(wallets: &[Wallet]) -> String {
    if wallets.is_empty() {
        return "No wallets found. Create one with 'create-wallet <name>'".to_string();
    }
    let mut text = Text::new();
    text.line("Wallets:");
    for wallet in wallets {
        text.line(wallet.terminal_display());
    }
    text.finish()
}

pub(crate) fn display_wallet(wallet: &Wallet) -> String {
    wallet.terminal_display()
}

pub(crate) fn removed_wallet(name: &str) -> String {
    format!("Removed wallet '{name}'")
}

pub(crate) fn submitted_tx(kind: &str, submitted: &SubmittedTx) -> String {
    let mut text = Text::new();
    text.line(format!("{kind} transaction committed"));
    text.line(format!("tx_hash: {}", submitted.tx_hash));
    text.line(format!("fee: {}", submitted.fee.amount()));
    text.line(format!("nonce: {}", submitted.nonce.value()));
    for event in deliver_tx_events(&submitted.response) {
        text.blank();
        text.line(format!("Event Type: {}", event.event_type));
        for (key, value) in event.attributes {
            text.line(format!("{key}: {value}"));
        }
    }
    text.finish()
}

pub(crate) fn faucet_ok(body: &str) -> String {
    match serde_json::from_str::<Value>(body) {
        Ok(value) => {
            let message = value
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("(no message)");
            match value.get("success").and_then(|v| v.as_bool()) {
                Some(success) => {
                    format!("Faucet request succeeded\nsuccess: {success}\nmessage: {message}")
                }
                None => format!("Faucet request succeeded\nmessage: {message}"),
            }
        }
        Err(_) => "Faucet request succeeded".to_string(),
    }
}

pub(crate) fn account(account: &Account) -> String {
    format!("Account found:\n{}", account.sanitized_log())
}

pub(crate) fn staking_account(address: &str, account: &StakingAccount) -> String {
    format!(
        "address: {address}\nstaking_account: {}",
        account.sanitized_log()
    )
}

pub(crate) async fn active_validators(
    cli: &ChainClient,
    node_url: &str,
    validators: Option<ActiveValidatorsInfo>,
) -> String {
    let mut text = Text::new();
    text.line(format!("Fetching active validators from {node_url}..."));
    match validators {
        Some(active_validators) => {
            if active_validators.validators.is_empty() {
                text.line("No active validators found in the current epoch");
                return text.finish();
            }

            text.line(format!(
                "Current Epoch: {}",
                active_validators.current_epoch
            ));
            text.line(format!("Total Stake: {}", active_validators.total_stake));
            text.blank();
            text.line(format!(
                "Active Validators (sorted by voting power): {}",
                active_validators.validators.len()
            ));

            for (i, validator) in active_validators.validators.iter().enumerate() {
                text.blank();
                text.line(format!("Validator #{}", i + 1));
                text.line(format!(
                    "  Address: {}",
                    SanitizedLog::as_address(validator.address)
                ));
                text.line(format!("  Stake (Voting Power): {}", validator.stake));

                let balance = match cli
                    .get_account_by_address(validator.address.to_string())
                    .await
                {
                    Ok(Some(account)) => account.balance().to_string(),
                    Ok(None) => "Account not found".to_string(),
                    Err(e) => format!("Error fetching account: {e}"),
                };
                text.line(format!("  Liquid Balance: {balance}"));
                text.line(format!(
                    "  Public Key: {}",
                    SanitizedLog::as_public_key(hex::encode(&validator.public_key))
                ));
            }
        }
        None => {
            text.line("No active validators information available");
        }
    }
    text.finish()
}

pub(crate) fn epoch(epoch_info: &EpochInfo, active_validators: &ActiveValidatorsInfo) -> String {
    let mut text = Text::new();
    text.line("╔══════════════════════════════════════════╗");
    text.line("║             EPOCH INFORMATION            ║");
    text.line("╚══════════════════════════════════════════╝");
    text.line(format!("  Current Epoch: {}", epoch_info.current_epoch));
    text.line(format!("  Current Block: {}", epoch_info.current_block));
    text.line(format!(
        "  Blocks Until Next Epoch: {}",
        epoch_info.blocks_until_next_epoch
    ));
    text.blank();

    text.line("╔══════════════════════════════════════════╗");
    text.line(format!(
        "║      ACTIVE VALIDATORS (EPOCH {})      ║",
        epoch_info.current_epoch
    ));
    text.line("╚══════════════════════════════════════════╝");
    text.line(format!("  Total Stake: {}", active_validators.total_stake));
    text.line(format!(
        "  Validators per Epoch: {}",
        epoch_info.validators_per_epoch
    ));
    text.blank();

    for (i, validator) in active_validators.validators.iter().enumerate() {
        let address_display = validator.address.to_string();
        let prefix_len = address_display.len().min(12);
        text.line(format!(
            "  Validator #{} - {}",
            i + 1,
            &address_display[0..prefix_len]
        ));
        text.line(format!("  ├─ Address: {address_display}"));
        let percentage = validator
            .stake
            .ratio(active_validators.total_stake)
            .map(|r| r * 100.0)
            .unwrap_or(0.0);
        text.line(format!(
            "  ├─ Stake: {} ({percentage:.2}% of total)",
            validator.stake
        ));
        let pk_hex = hex::encode(&validator.public_key);
        let pk_prefix_len = pk_hex.len().min(16);
        text.line(format!("  └─ Public Key: {}...", &pk_hex[0..pk_prefix_len]));
        text.blank();
    }

    text.line("╔══════════════════════════════════════════╗");
    text.line("║               EPOCH TIMER                ║");
    text.line("╚══════════════════════════════════════════╝");
    text.line(format!(
        "  Next validator rotation in {} blocks",
        epoch_info.blocks_until_next_epoch
    ));

    let progress = ((epoch_info.blocks_per_epoch - epoch_info.blocks_until_next_epoch) as f64
        / epoch_info.blocks_per_epoch as f64)
        * 100.0;
    let bar_width = 50;
    let filled_width = (progress / 100.0 * bar_width as f64) as usize;
    let mut bar = String::from("  [");
    for i in 0..bar_width {
        if i < filled_width {
            bar.push('█');
        } else {
            bar.push('░');
        }
    }
    bar.push_str(&format!("] {progress:.1}%"));
    text.line(bar);
    text.finish()
}

pub(crate) fn namespace_lookup(lookup: &NamespaceLookup) -> String {
    match &lookup.registered {
        Some(resp) => print_registered(resp),
        None => format!(
            "registered: false\nnamespace_slug: {}",
            lookup.canonical_slug
        ),
    }
}

pub(crate) fn print_registered(resp: &NamespaceRegisteredResponse) -> String {
    format!(
        "registered: {}\nnamespace_slug: {}\nscope: {}\nowner: {}\nregistered_height: {}\nregistry_path: {}",
        resp.registered,
        resp.namespace_slug,
        resp.scope,
        resp.owner,
        resp.registered_height,
        resp.registry_path
    )
}

pub(crate) fn pinboard_submit(resp: &PostMessageSubmitResponse) -> String {
    let mut text = Text::new();
    text.line(format!("status: {}", resp.status));
    text.line(format!("message_id: {}", resp.message_id));
    text.line(format!("content_key: {}", resp.content_key));
    text.line(format!("tx_hash: {}", resp.tx_hash));
    text.line(format!("origin_validator: {}", resp.origin_validator));
    text.line(format!("received_timestamp: {}", resp.received_timestamp));
    if let Some(content_path) = &resp.content_path {
        text.line(format!("content_path: {content_path}"));
    }
    text.finish()
}

pub(crate) fn pinboard_post(path: &str, v: &Value) -> String {
    let mut text = Text::new();
    text.line(format!("Pinboard REST response for {path}:"));
    text.line(v.to_string());

    if let Some(message_b64) = v.get("message_b64").and_then(|m| m.as_str()) {
        match BASE64_STANDARD.decode(message_b64.as_bytes()) {
            Ok(decoded) => match String::from_utf8(decoded.clone()) {
                Ok(message) => {
                    text.line("Pinboard decoded message:");
                    text.line(message);
                }
                Err(_) => text.line(format!(
                    "Pinboard decoded message (hex): 0x{}",
                    hex::encode(decoded)
                )),
            },
            Err(e) => text.line(format!("Failed to decode pinboard message_b64: {e}")),
        }
    } else {
        let blob_status = v
            .get("blob_status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown");
        text.line(format!(
            "Pinboard REST response did not include message_b64 (blob_status={blob_status})"
        ));
    }
    text.finish()
}

pub(crate) fn pinboard_list(path: &str, v: &Value) -> String {
    format!("Pinboard response for {path}:\n{v}")
}

pub(crate) fn list_cados(search_string: &str, paths: &[String]) -> String {
    let mut text = Text::new();
    text.line(format!(
        "Found {} CADO paths matching '{search_string}':",
        paths.len()
    ));
    for (i, path) in paths.iter().enumerate() {
        text.line(format!("{}. {path}", i + 1));
    }
    text.finish()
}

fn json_bytes(values: &[Value]) -> Vec<u8> {
    values
        .iter()
        .filter_map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
        .collect()
}

pub(crate) fn cado(path: &str, response: &Value) -> Result<String, EldError> {
    let cado = if let Some(mutable) = response.get("Mutable") {
        Some(("Mutable", mutable))
    } else {
        response
            .get("Immutable")
            .map(|immutable| ("Immutable", immutable))
    };

    let Some((cado_type, cado)) = cado else {
        return Err(ErrorBuilder::not_found_error("CADO", path));
    };

    let mut text = Text::new();
    text.line("CADO query response:");
    text.blank();
    text.line(format!("CADO Type: {cado_type}"));

    if let Some(metadata) = cado.get("metadata") {
        text.blank();
        text.line("Metadata:");
        text.line(format!(
            "  Type:\t\t {}",
            metadata.get("type_").unwrap_or(&serde_json::Value::Null)
        ));
        text.line(format!(
            "  Owner:\t {}",
            metadata.get("owner").unwrap_or(&serde_json::Value::Null)
        ));
    }

    if let Some(hash) = cado.get("hash") {
        if let Some(hash_array) = hash.as_array() {
            text.blank();
            text.line(format!(
                "Hash:\t\t 0x{}",
                hex::encode(json_bytes(hash_array))
            ));
        }
    }

    if cado_type == "Mutable" {
        if let Some(latest_hash) = cado.get("latest_hash") {
            if let Some(hash_array) = latest_hash.as_array() {
                text.line(format!(
                    "Latest Hash:\t 0x{}",
                    hex::encode(json_bytes(hash_array))
                ));
            }
        }
    }

    let Some(data) = cado.get("data").and_then(|d| d.as_array()) else {
        return Ok(text.finish());
    };
    let data_bytes = json_bytes(data);

    if path.starts_with(PATH_PREFIX_STAKING_ACCOUNT) {
        match StakingAccount::deserialize_bin(&data_bytes) {
            Ok(account) => {
                text.blank();
                text.line("Account Details:");
                text.line(format!("  Originator:\t {}", account.originator));
                text.line(format!("  Balance:\t {}", account.stake_balance));
            }
            Err(e) => {
                text.blank();
                text.line(format!("Failed to parse account data: {e}"));
                text.line("Raw data (hex):");
                text.line(format!("  0x{}", hex::encode(data_bytes)));
            }
        }
    } else if path.starts_with(PATH_PREFIX_ACCOUNT) || path.starts_with(PATH_PREFIX_CADO_MAP) {
        match Account::deserialize_bin(&data_bytes) {
            Ok(account) => {
                text.blank();
                text.line("Account Details:");
                text.line(format!("  Address: {}", account.address()));
                text.line(format!("  Balance: {}", account.balance()));
                text.line(format!("  Nonce: {}", account.nonce()));
            }
            Err(e) => {
                text.blank();
                text.line(format!("Failed to parse account data: {e}"));
                text.line("Raw data (hex):");
                text.line(format!("  0x{}", hex::encode(data_bytes)));
            }
        }
    } else if path.starts_with(PATH_PREFIX_ACCOUNT_CONTENT) {
        match bincode::deserialize::<Vec<String>>(&data_bytes) {
            Ok(manifest_ids) => {
                text.blank();
                text.line("Account Content Summary:");
                text.line(format!(
                    "  Address:\t {}",
                    path.split('/').next_back().unwrap_or("unknown")
                ));
                text.line(format!(
                    "  Total Content Manifests:\t {}",
                    manifest_ids.len()
                ));
                if manifest_ids.is_empty() {
                    text.line("  No content manifests found");
                } else {
                    text.line("  Content Manifest IDs:");
                    for (index, manifest_id) in manifest_ids.iter().enumerate() {
                        text.line(format!("     {}. {manifest_id}", index + 1));
                    }
                }
            }
            Err(e) => {
                text.blank();
                text.line(format!("Failed to parse account content data: {e}"));
                text.line("Raw data (hex):");
                text.line(format!("  0x{}", hex::encode(data_bytes)));
            }
        }
    } else if path.starts_with(PATH_PREFIX_APP_STATE_SNAPSHOT) {
        text.line("App State Snapshot Details:");
        text.line(format!("* Snapshot Path: \t{path}"));
        text.line(format!(
            "* Total Size: \t{} bytes ({:.2} KB)",
            data_bytes.len(),
            data_bytes.len() as f64 / 1024.0
        ));

        if data_bytes.len() >= 8 {
            let block_height_bytes = &data_bytes[0..8];
            if let Ok(block_height_array) = block_height_bytes.try_into() {
                let block_height = i64::from_le_bytes(block_height_array);
                text.line(format!("* Block Height: \t{block_height}"));
            }
        }

        if data_bytes.len() >= 40 {
            let root_hash = &data_bytes[8..40];
            text.line(format!("* Root Hash: \t0x{}", hex::encode(root_hash)));
        }

        if data_bytes.len() >= 48 {
            let node_count_bytes = &data_bytes[40..48];
            if let Ok(node_count_array) = node_count_bytes.try_into() {
                let node_count = usize::from_le_bytes(node_count_array);
                text.line(format!("* Node Count: \t{node_count}"));
                if node_count > 0 {
                    text.line(format!(
                        "* Avg bytes/node: \t{:.2}",
                        data_bytes.len() as f64 / node_count as f64
                    ));
                }
            }
        }

        if data_bytes.len() >= 56 {
            let timestamp_bytes = &data_bytes[48..56];
            if let Ok(timestamp_array) = timestamp_bytes.try_into() {
                let timestamp = u64::from_le_bytes(timestamp_array);
                text.line(format!("* Timestamp: \t{timestamp} (Unix)"));
            }
        }

        text.line("Trie snapshot found and can be restored on node startup");
        text.line("Use './start_app_with_db_data.sh' to restore from this snapshot");
    } else if let Ok(str_data) = String::from_utf8(data_bytes.clone()) {
        text.line("Data (as string):");
        text.line(format!("* {str_data}"));
    } else {
        text.line("Data (as hex):");
        text.line(format!("* 0x{}", hex::encode(data_bytes)));
    }

    Ok(text.finish())
}
