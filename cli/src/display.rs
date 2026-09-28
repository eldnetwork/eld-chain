//! Human-facing CLI output previously printed inside `eld-client`.

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
use eld_common::tx::Tx;
use eld_common::validator::{ActiveValidatorsInfo, EpochInfo};
use eld_common::wallet::Wallet;
use serde_json::Value;
use tendermint::abci::EventAttribute;
use tracing::{info, warn};

pub fn log_deliver_tx_events(response: &Value) {
    for event in deliver_tx_events(response) {
        info!("\nEvent Type: {}", event.event_type);
        for (key, value) in event.attributes {
            info!("{key}: {value}");
        }
    }
}

pub fn created_wallet(wallet: &Wallet) {
    info!(wallet_name = %wallet.name, "Created wallet");
    info!("{}", wallet.terminal_display());
}

pub fn list_wallets(wallets: &[Wallet]) {
    info!("Listing wallets");
    info!("Wallets:\n");
    if wallets.is_empty() {
        info!("No wallets found");
        info!("No wallets found. Create one with 'create-wallet <name>'");
    } else {
        info!(wallet_count = wallets.len(), "Retrieved wallets");
        for wallet in wallets {
            info!("{}", wallet.terminal_display());
        }
    }
}

pub fn display_wallet(name: &str, wallet: Option<&Wallet>) {
    if let Some(wallet) = wallet {
        info!(wallet_name = %name, "Displaying wallet");
        info!("{}", wallet.terminal_display());
    } else {
        warn!(wallet_name = %name, "Couldn't find wallet for display");
        warn!("Couldn't find wallet");
    }
}

pub fn removed_wallet(name: &str, removed: bool) {
    if !removed {
        warn!(wallet_name = %name, "Wallet not found for removal");
        warn!("Wallet with name '{name}' not found");
    }
}

pub fn submitted_tx(kind: &str, submitted: &SubmittedTx) {
    info!(
        tx_hash = %submitted.tx_hash,
        fee = %submitted.fee.amount(),
        nonce = submitted.nonce.value(),
        "{kind} transaction committed"
    );
    log_deliver_tx_events(&submitted.response);
}

pub fn faucet_ok(body: &str) {
    match serde_json::from_str::<Value>(body) {
        Ok(value) => {
            let success = value.get("success").and_then(|v| v.as_bool());
            let message = value
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("(no message)");
            info!(?success, message, "Faucet request succeeded");
        }
        Err(_) => info!("Faucet request succeeded"),
    }
}

pub fn account(address: &str, account: Option<&Account>) {
    match account {
        Some(account) => {
            info!("Account found:");
            info!("{}", account.sanitized_log());
        }
        None => {
            warn!(
                "No account found for address: {}",
                SanitizedLog::as_address(address)
            );
        }
    }
}

pub fn staking_account(address: &str, account: &eld_common::staking_account::StakingAccount) {
    info!(address = %address, "Retrieved staking account");
    info!("staking_account: {}", account.sanitized_log());
}

pub fn all_transactions(txs: &[Tx]) {
    info!("Transactions:\n");
    for tx in txs {
        info!("tx: {}", SanitizedLog::new(tx.clone()));
    }
}

fn decode_event_attribute(attribute: EventAttribute) -> Result<(String, String), EldError> {
    let key_str = attribute
        .key_str()
        .map_err(|e| ErrorBuilder::validation_error("event_attribute_key", "", &e.to_string()))?;
    let decoded_key_bytes = BASE64_STANDARD.decode(key_str).map_err(|e| {
        ErrorBuilder::validation_error("event_attribute_key", key_str, &e.to_string())
    })?;
    let key = String::from_utf8(decoded_key_bytes).map_err(|e| {
        ErrorBuilder::validation_error("event_attribute_key", key_str, &e.to_string())
    })?;

    let value_str = attribute
        .value_str()
        .map_err(|e| ErrorBuilder::validation_error("event_attribute_value", "", &e.to_string()))?;
    let decoded_value_bytes = BASE64_STANDARD.decode(value_str).map_err(|e| {
        ErrorBuilder::validation_error("event_attribute_value", value_str, &e.to_string())
    })?;
    let value = String::from_utf8(decoded_value_bytes).map_err(|e| {
        ErrorBuilder::validation_error("event_attribute_value", value_str, &e.to_string())
    })?;
    Ok((key, value))
}

pub fn account_transactions(
    txs: Vec<tendermint_rpc::endpoint::tx::Response>,
) -> Result<(), EldError> {
    info!("\nTxs:\n");
    for tx_response in txs {
        info!(
            "tx hash: {}",
            SanitizedLog::as_hash(tx_response.hash.to_string())
        );
        for event in tx_response.tx_result.events {
            info!("event kind: {}", event.kind);
            for attribute in event.attributes {
                let (key, value) = decode_event_attribute(attribute)?;
                info!("{key}:{value}");
            }
        }
        info!("\n");
    }
    Ok(())
}

pub async fn active_validators(
    cli: &ChainClient,
    node_url: &str,
    validators: Option<ActiveValidatorsInfo>,
) -> Result<(), EldError> {
    info!("Fetching active validators from {node_url}...");
    match validators {
        Some(active_validators) => {
            if active_validators.validators.is_empty() {
                info!("No active validators found in the current epoch");
                return Ok(());
            }

            info!("Current Epoch: {}", active_validators.current_epoch);
            info!("Total Stake: {}", active_validators.total_stake);
            info!(
                "\nActive Validators (sorted by voting power): {}",
                active_validators.validators.len()
            );

            for (i, validator) in active_validators.validators.iter().enumerate() {
                info!("\nValidator #{}", i + 1);
                info!("  Address: {}", SanitizedLog::as_address(validator.address));
                info!("  Stake (Voting Power): {}", validator.stake);

                match cli
                    .get_account_by_address(validator.address.to_string())
                    .await
                {
                    Ok(Some(account)) => {
                        info!("  Liquid Balance: {}", account.balance());
                    }
                    Ok(None) => {
                        info!("  Liquid Balance: Account not found");
                    }
                    Err(e) => {
                        warn!("  Liquid Balance: Error fetching account: {}", e);
                    }
                }

                info!(
                    "  Public Key: {}",
                    SanitizedLog::as_public_key(hex::encode(&validator.public_key))
                );
            }
            Ok(())
        }
        None => {
            info!("No active validators information available");
            Ok(())
        }
    }
}

pub fn epoch_info(epoch_info: &EpochInfo) {
    info!("Current Epoch: {}", epoch_info.current_epoch);
    info!("Current Block: {}", epoch_info.current_block);
    info!("Blocks Per Epoch: {}", epoch_info.blocks_per_epoch);
    info!("Validators Per Epoch: {}", epoch_info.validators_per_epoch);
    info!(
        "Blocks Until Next Epoch: {}",
        epoch_info.blocks_until_next_epoch
    );
}

pub fn epoch(epoch_info: &EpochInfo, active_validators: &ActiveValidatorsInfo) {
    info!("╔══════════════════════════════════════════╗");
    info!("║             EPOCH INFORMATION            ║");
    info!("╚══════════════════════════════════════════╝");
    info!("  Current Epoch: {}", epoch_info.current_epoch);
    info!("  Current Block: {}", epoch_info.current_block);
    info!(
        "  Blocks Until Next Epoch: {}",
        epoch_info.blocks_until_next_epoch
    );
    info!("");

    info!("╔══════════════════════════════════════════╗");
    info!(
        "║      ACTIVE VALIDATORS (EPOCH {})      ║",
        epoch_info.current_epoch
    );
    info!("╚══════════════════════════════════════════╝");
    info!("  Total Stake: {}", active_validators.total_stake);
    info!(
        "  Validators per Epoch: {}",
        epoch_info.validators_per_epoch
    );
    info!("");

    for (i, validator) in active_validators.validators.iter().enumerate() {
        let address_display = validator.address.to_string();
        let prefix_len = address_display.len().min(12);
        info!(
            "  Validator #{} - {}",
            i + 1,
            &address_display[0..prefix_len]
        );
        info!("  ├─ Address: {}", address_display);
        let percentage = validator
            .stake
            .ratio(active_validators.total_stake)
            .map(|r| r * 100.0)
            .unwrap_or(0.0);
        info!(
            "  ├─ Stake: {} ({:.2}% of total)",
            validator.stake, percentage
        );
        let pk_hex = hex::encode(&validator.public_key);
        let pk_prefix_len = pk_hex.len().min(16);
        info!("  └─ Public Key: {}...", &pk_hex[0..pk_prefix_len]);
        info!("");
    }

    info!("╔══════════════════════════════════════════╗");
    info!("║               EPOCH TIMER                ║");
    info!("╚══════════════════════════════════════════╝");
    info!(
        "  Next validator rotation in {} blocks",
        epoch_info.blocks_until_next_epoch
    );

    let progress = ((epoch_info.blocks_per_epoch - epoch_info.blocks_until_next_epoch) as f64
        / epoch_info.blocks_per_epoch as f64)
        * 100.0;

    let bar_width = 50;
    let filled_width = (progress / 100.0 * bar_width as f64) as usize;

    info!("  [");
    for i in 0..bar_width {
        if i < filled_width {
            info!("█");
        } else {
            info!("░");
        }
    }
    info!("] {:.1}%", progress);
}

pub fn namespace_lookup(lookup: &NamespaceLookup) {
    match &lookup.registered {
        Some(resp) => print_registered(resp),
        None => {
            println!("registered: false");
            println!("namespace_slug: {}", lookup.canonical_slug);
        }
    }
}

pub fn print_registered(resp: &NamespaceRegisteredResponse) {
    info!(
        namespace_slug = %resp.namespace_slug,
        owner = %resp.owner,
        registered_height = resp.registered_height,
        registry_path = %resp.registry_path,
        "Namespace registered"
    );
    println!("registered: {}", resp.registered);
    println!("namespace_slug: {}", resp.namespace_slug);
    println!("scope: {}", resp.scope);
    println!("owner: {}", resp.owner);
    println!("registered_height: {}", resp.registered_height);
    println!("registry_path: {}", resp.registry_path);
}

pub fn pinboard_submit(resp: &PostMessageSubmitResponse) {
    info!(
        message_id = %resp.message_id,
        tx_hash = %resp.tx_hash,
        "Pinboard message accepted by node"
    );
    println!("status: {}", resp.status);
    println!("message_id: {}", resp.message_id);
    println!("content_key: {}", resp.content_key);
    println!("tx_hash: {}", resp.tx_hash);
    println!("origin_validator: {}", resp.origin_validator);
    println!("received_timestamp: {}", resp.received_timestamp);
    if let Some(content_path) = &resp.content_path {
        println!("content_path: {content_path}");
    }
}

pub fn pinboard_post(path: &str, v: &Value) {
    info!("Pinboard REST response for {}:\n{}", path, v);

    if let Some(message_b64) = v.get("message_b64").and_then(|m| m.as_str()) {
        match BASE64_STANDARD.decode(message_b64.as_bytes()) {
            Ok(decoded) => match String::from_utf8(decoded.clone()) {
                Ok(text) => info!("Pinboard decoded message:\n{}", text),
                Err(_) => {
                    info!("Pinboard decoded message (hex): 0x{}", hex::encode(decoded))
                }
            },
            Err(e) => warn!("Failed to decode pinboard message_b64: {}", e),
        }
    } else {
        let blob_status = v
            .get("blob_status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown");
        warn!(
            "Pinboard REST response did not include message_b64 (blob_status={})",
            blob_status
        );
    }
}

pub fn pinboard_list(path: &str, v: &Value) {
    info!("Pinboard response for {}:\n{}", path, v);
}

pub fn list_cados(search_string: &str, paths: &[String]) {
    info!(
        "Found {} CADO paths matching '{}':",
        paths.len(),
        search_string
    );
    for (i, path) in paths.iter().enumerate() {
        info!("{}. {}", i + 1, path);
    }
}

fn json_bytes(values: &[Value]) -> Vec<u8> {
    values
        .iter()
        .filter_map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
        .collect()
}

pub fn cado(path: &str, response: &Value) {
    info!("CADO query response:");
    let cado = if let Some(mutable) = response.get("Mutable") {
        Some(("Mutable", mutable))
    } else {
        response
            .get("Immutable")
            .map(|immutable| ("Immutable", immutable))
    };

    let Some((cado_type, cado)) = cado else {
        info!("No CADO found at path: {}", path);
        return;
    };

    info!("\nCADO Type: {}", cado_type);

    if let Some(metadata) = cado.get("metadata") {
        info!("\nMetadata:");
        info!(
            "  Type:\t\t {}",
            metadata.get("type_").unwrap_or(&serde_json::Value::Null)
        );
        info!(
            "  Owner:\t {}",
            metadata.get("owner").unwrap_or(&serde_json::Value::Null)
        );
    }

    if let Some(hash) = cado.get("hash") {
        if let Some(hash_array) = hash.as_array() {
            info!("\nHash:\t\t 0x{}", hex::encode(json_bytes(hash_array)));
        }
    }

    if cado_type == "Mutable" {
        if let Some(latest_hash) = cado.get("latest_hash") {
            if let Some(hash_array) = latest_hash.as_array() {
                info!("Latest Hash:\t 0x{}", hex::encode(json_bytes(hash_array)));
            }
        }
    }

    let Some(data) = cado.get("data").and_then(|d| d.as_array()) else {
        return;
    };
    let data_bytes = json_bytes(data);

    if path.starts_with(PATH_PREFIX_STAKING_ACCOUNT) {
        match StakingAccount::deserialize_bin(&data_bytes) {
            Ok(account) => {
                info!("\nAccount Details:");
                info!("  Originator:\t {}", account.originator);
                info!("  Balance:\t {}", account.stake_balance);
            }
            Err(e) => {
                info!("\nFailed to parse account data: {}", e);
                info!("Raw data (hex):");
                info!("  0x{}", hex::encode(data_bytes));
            }
        }
    } else if path.starts_with(PATH_PREFIX_ACCOUNT) || path.starts_with(PATH_PREFIX_CADO_MAP) {
        match Account::deserialize_bin(&data_bytes) {
            Ok(account) => {
                info!("\nAccount Details:");
                info!("  Address: {}", account.address());
                info!("  Balance: {}", account.balance());
                info!("  Nonce: {}", account.nonce());
            }
            Err(e) => {
                info!("\nFailed to parse account data: {}", e);
                info!("Raw data (hex):");
                info!("  0x{}", hex::encode(data_bytes));
            }
        }
    } else if path.starts_with(PATH_PREFIX_ACCOUNT_CONTENT) {
        match bincode::deserialize::<Vec<String>>(&data_bytes) {
            Ok(manifest_ids) => {
                info!("\nAccount Content Summary:");
                info!(
                    "  Address:\t {}",
                    path.split('/').next_back().unwrap_or("unknown")
                );
                info!("  Total Content Manifests:\t {}", manifest_ids.len());

                if manifest_ids.is_empty() {
                    info!("  No content manifests found");
                } else {
                    info!("  Content Manifest IDs:");
                    for (index, manifest_id) in manifest_ids.iter().enumerate() {
                        info!("     {}. {}", index + 1, manifest_id);
                    }
                }
            }
            Err(e) => {
                info!("\nFailed to parse account content data: {}", e);
                info!("Raw data (hex):");
                info!("  0x{}", hex::encode(data_bytes));
            }
        }
    } else if path.starts_with(PATH_PREFIX_APP_STATE_SNAPSHOT) {
        info!("App State Snapshot Details:\n");
        info!("* Snapshot Path: \t{}", path);
        info!(
            "* Total Size: \t{} bytes ({:.2} KB)",
            data_bytes.len(),
            data_bytes.len() as f64 / 1024.0
        );

        if data_bytes.len() >= 8 {
            let block_height_bytes = &data_bytes[0..8];
            if let Ok(block_height_array) = block_height_bytes.try_into() {
                let block_height = i64::from_le_bytes(block_height_array);
                info!("* Block Height: \t{}", block_height);
            }
        }

        if data_bytes.len() >= 40 {
            let root_hash = &data_bytes[8..40];
            info!("* Root Hash: \t0x{}", hex::encode(root_hash));
        }

        if data_bytes.len() >= 48 {
            let node_count_bytes = &data_bytes[40..48];
            if let Ok(node_count_array) = node_count_bytes.try_into() {
                let node_count = usize::from_le_bytes(node_count_array);
                info!("* Node Count: \t{}", node_count);
                if node_count > 0 {
                    info!(
                        "* Avg bytes/node: \t{:.2}",
                        data_bytes.len() as f64 / node_count as f64
                    );
                }
            }
        }

        if data_bytes.len() >= 56 {
            let timestamp_bytes = &data_bytes[48..56];
            if let Ok(timestamp_array) = timestamp_bytes.try_into() {
                let timestamp = u64::from_le_bytes(timestamp_array);
                info!("* Timestamp: \t{} (Unix)", timestamp);
            }
        }

        info!("Trie snapshot found and can be restored on node startup");
        info!("Use './start_app_with_db_data.sh' to restore from this snapshot");
    } else if let Ok(str_data) = String::from_utf8(data_bytes.clone()) {
        info!("Data (as string):");
        info!("* {}", str_data);
    } else {
        info!("Data (as hex):");
        info!("* 0x{}", hex::encode(data_bytes));
    }
}
