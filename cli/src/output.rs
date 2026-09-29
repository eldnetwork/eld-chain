//! Human-facing CLI text. Callers print the returned strings on stdout.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use eld_client::api::abci::{deliver_tx_events, AbciInfoWrapper};
use eld_client::api::rest::{NamespaceRegisteredResponse, PostMessageSubmitResponse};
use eld_client::facade::NamespaceLookup;
use eld_client::facade::SubmittedTx;
use eld_common::account::Account;
use eld_common::constants::cado::{
    PATH_PREFIX_ACCOUNT, PATH_PREFIX_ACCOUNT_CONTENT, PATH_PREFIX_APP_STATE_SNAPSHOT,
    PATH_PREFIX_CADO_MAP, PATH_PREFIX_STAKING_ACCOUNT,
};
use eld_common::error::{EldError, ErrorBuilder};
use eld_common::staking_account::StakingAccount;
use eld_common::utils::json_number_array_as_bytes;
use eld_common::validator::{ActiveValidatorsInfo, EpochInfo};
use eld_common::wallet::Wallet;
use serde::Serialize;
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct OutputMode {
    pub(crate) format: crate::args::OutputFormat,
}

impl OutputMode {
    pub(crate) fn new(format: crate::args::OutputFormat) -> Self {
        Self { format }
    }

    #[cfg(test)]
    pub(crate) fn text() -> Self {
        Self::new(crate::args::OutputFormat::Text)
    }

    pub(crate) fn is_json(self) -> bool {
        self.format == crate::args::OutputFormat::Json
    }
}

pub(crate) fn print_result(text: &str) {
    let text = text.trim_end_matches('\n');
    if text.is_empty() {
        return;
    }
    println!("{text}");
}

pub(crate) const UNENCRYPTED_WALLET_WARNING: &str = "Warning: local wallets store unencrypted Ed25519 keys. Keep wallets.json mode 0600 and never commit it.";

pub(crate) fn print_json<T: Serialize>(value: &T) -> Result<(), EldError> {
    let json = serde_json::to_string(value).map_err(|err| EldError::ValidationError {
        field: "output".to_string(),
        value: "json".to_string(),
        details: format!("failed to serialize JSON: {err}"),
    })?;
    println!("{json}");
    Ok(())
}

pub(crate) fn emit<T: Serialize>(mode: OutputMode, text: &str, value: &T) -> Result<(), EldError> {
    if mode.is_json() {
        print_json(value)
    } else {
        print_result(text);
        Ok(())
    }
}

pub(crate) fn warn_unencrypted_wallets() {
    eprintln!("{UNENCRYPTED_WALLET_WARNING}");
}

pub(crate) fn dry_run_transfer(wallet: &str, recipient: &str, amount: u128) -> String {
    format!("dry-run: transfer\nwallet: {wallet}\nrecipient: {recipient}\namount: {amount}")
}

pub(crate) fn dry_run_stake(wallet: &str, amount: u128) -> String {
    format!("dry-run: stake\nwallet: {wallet}\namount: {amount}")
}

pub(crate) fn dry_run_unstake(wallet: &str, amount: u128) -> String {
    format!("dry-run: unstake\nwallet: {wallet}\namount: {amount}")
}

pub(crate) fn dry_run_add_namespace(
    wallet: &str,
    namespace: &str,
    registration_fee: u128,
) -> String {
    format!(
        "dry-run: add-namespace\nwallet: {wallet}\nnamespace: {namespace}\nregistration_fee: {registration_fee}"
    )
}

pub(crate) fn dry_run_pinboard_post(
    wallet: &str,
    file_path: &str,
    user_fee_amount: u128,
) -> String {
    format!(
        "dry-run: pinboard post\nwallet: {wallet}\nfile: {file_path}\nuser_fee_amount: {user_fee_amount}"
    )
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
        text.line(format!("event_type: {}", event.event_type));
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
    format!(
        "address: {}\nbalance: {}\nnonce: {}",
        account.address().hex_with_prefix(),
        account.balance(),
        account.nonce()
    )
}

pub(crate) fn staking_account(address: &str, account: &StakingAccount) -> String {
    format!(
        "address: {address}\nstake_balance: {}\noriginator: {}",
        account.stake_balance, account.originator
    )
}

pub(crate) fn active_validators(
    validators: Option<&ActiveValidatorsInfo>,
    balances: &[String],
) -> String {
    let mut text = Text::new();
    match validators {
        Some(active_validators) => {
            if active_validators.validators.is_empty() {
                text.line("No active validators in the current epoch");
                return text.finish();
            }

            text.line(format!(
                "Current epoch: {}",
                active_validators.current_epoch
            ));
            text.line(format!("Total stake: {}", active_validators.total_stake));
            text.blank();
            text.line(format!(
                "Active validators: {}",
                active_validators.validators.len()
            ));

            for (i, validator) in active_validators.validators.iter().enumerate() {
                text.blank();
                text.line(format!("Validator {}", i + 1));
                text.line(format!("  address: {}", validator.address));
                text.line(format!("  stake: {}", validator.stake));
                let balance = balances.get(i).map(String::as_str).unwrap_or("unknown");
                text.line(format!("  balance: {balance}"));
                text.line(format!(
                    "  public_key: {}",
                    hex::encode(&validator.public_key)
                ));
            }
        }
        None => {
            text.line("No active validator information");
        }
    }
    text.finish()
}

pub(crate) fn epoch(epoch_info: &EpochInfo, active_validators: &ActiveValidatorsInfo) -> String {
    let mut text = Text::new();
    text.line("Epoch");
    text.line(format!("  current_epoch: {}", epoch_info.current_epoch));
    text.line(format!("  current_block: {}", epoch_info.current_block));
    text.line(format!(
        "  blocks_until_next_epoch: {}",
        epoch_info.blocks_until_next_epoch
    ));
    text.blank();

    text.line(format!(
        "Active validators (epoch {})",
        epoch_info.current_epoch
    ));
    text.line(format!("  total_stake: {}", active_validators.total_stake));
    text.line(format!(
        "  validators_per_epoch: {}",
        epoch_info.validators_per_epoch
    ));
    text.blank();

    for (i, validator) in active_validators.validators.iter().enumerate() {
        let address_display = validator.address.to_string();
        text.line(format!("Validator {}", i + 1));
        text.line(format!("  address: {address_display}"));
        let percentage = validator
            .stake
            .ratio(active_validators.total_stake)
            .map(|r| r * 100.0)
            .unwrap_or(0.0);
        text.line(format!(
            "  stake: {} ({percentage:.2}% of total)",
            validator.stake
        ));
        text.line(format!(
            "  public_key: {}",
            hex::encode(&validator.public_key)
        ));
        text.blank();
    }

    text.line(format!(
        "Next validator rotation in {} blocks",
        epoch_info.blocks_until_next_epoch
    ));

    let progress = if epoch_info.blocks_per_epoch == 0 {
        0.0
    } else {
        ((epoch_info.blocks_per_epoch - epoch_info.blocks_until_next_epoch) as f64
            / epoch_info.blocks_per_epoch as f64)
            * 100.0
    };
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
    text.line(format!("Pinboard post {path}:"));
    text.line(v.to_string());

    if let Some(message_b64) = v.get("message_b64").and_then(|m| m.as_str()) {
        match BASE64_STANDARD.decode(message_b64.as_bytes()) {
            Ok(decoded) => match String::from_utf8(decoded.clone()) {
                Ok(message) => {
                    text.line("Message:");
                    text.line(message);
                }
                Err(_) => text.line(format!("Message (hex): 0x{}", hex::encode(decoded))),
            },
            Err(e) => text.line(format!("Failed to decode message: {e}")),
        }
    } else {
        let blob_status = v
            .get("blob_status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown");
        text.line(format!("No message body (blob_status={blob_status})"));
    }
    text.finish()
}

pub(crate) fn pinboard_list(path: &str, v: &Value) -> String {
    format!("Pinboard posts {path}:\n{v}")
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

#[derive(Serialize)]
struct WalletJson {
    name: String,
    address: String,
    public_key: String,
}

fn wallet_json(wallet: &Wallet) -> WalletJson {
    WalletJson {
        name: wallet.name.clone(),
        address: wallet.address.hex_with_prefix(),
        public_key: hex::encode(wallet.public_key),
    }
}

#[derive(Serialize)]
struct WalletListJson {
    wallets: Vec<WalletJson>,
}

#[derive(Serialize)]
struct RemovedWalletJson {
    name: String,
    removed: bool,
}

#[derive(Serialize)]
struct EventAttrJson {
    key: String,
    value: String,
}

#[derive(Serialize)]
struct EventJson {
    event_type: String,
    attributes: Vec<EventAttrJson>,
}

#[derive(Serialize)]
struct SubmittedTxJson {
    kind: String,
    tx_hash: String,
    fee: u128,
    nonce: u32,
    events: Vec<EventJson>,
}

fn submitted_tx_json(kind: &str, submitted: &SubmittedTx) -> SubmittedTxJson {
    SubmittedTxJson {
        kind: kind.to_string(),
        tx_hash: submitted.tx_hash.to_string(),
        fee: submitted.fee.amount(),
        nonce: submitted.nonce.value(),
        events: deliver_tx_events(&submitted.response)
            .into_iter()
            .map(|event| EventJson {
                event_type: event.event_type,
                attributes: event
                    .attributes
                    .into_iter()
                    .map(|(key, value)| EventAttrJson { key, value })
                    .collect(),
            })
            .collect(),
    }
}

#[derive(Serialize)]
struct FaucetJson {
    success: Option<bool>,
    message: String,
}

fn faucet_json(body: &str) -> FaucetJson {
    match serde_json::from_str::<Value>(body) {
        Ok(value) => FaucetJson {
            success: value.get("success").and_then(|v| v.as_bool()),
            message: value
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("(no message)")
                .to_string(),
        },
        Err(_) => FaucetJson {
            success: None,
            message: "Faucet request succeeded".to_string(),
        },
    }
}

#[derive(Serialize)]
struct AbciJson {
    app_version: u64,
    version: String,
    last_block_height: String,
    last_block_app_hash: String,
}

#[derive(Serialize)]
struct ValidatorJson {
    address: String,
    stake: String,
    balance: String,
    public_key: String,
}

#[derive(Serialize)]
struct ActiveValidatorsJson {
    current_epoch: Option<i64>,
    total_stake: Option<String>,
    validators: Vec<ValidatorJson>,
}

fn active_validators_json(
    info: Option<&ActiveValidatorsInfo>,
    balances: &[String],
) -> ActiveValidatorsJson {
    let Some(info) = info else {
        return ActiveValidatorsJson {
            current_epoch: None,
            total_stake: None,
            validators: Vec::new(),
        };
    };
    ActiveValidatorsJson {
        current_epoch: Some(info.current_epoch),
        total_stake: Some(info.total_stake.to_string()),
        validators: info
            .validators
            .iter()
            .enumerate()
            .map(|(i, validator)| ValidatorJson {
                address: validator.address.to_string(),
                stake: validator.stake.to_string(),
                balance: balances
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                public_key: hex::encode(&validator.public_key),
            })
            .collect(),
    }
}

#[derive(Serialize)]
struct EpochJson<'a> {
    epoch: &'a EpochInfo,
    validators: &'a ActiveValidatorsInfo,
}

#[derive(Serialize)]
struct MissingNamespaceJson {
    registered: bool,
    namespace_slug: String,
}

#[derive(Serialize)]
struct CadoListJson<'a> {
    search_string: &'a str,
    paths: &'a [String],
}

#[derive(Serialize)]
struct DryRunTransferJson<'a> {
    action: &'static str,
    wallet: &'a str,
    recipient: &'a str,
    amount: u128,
}

#[derive(Serialize)]
struct DryRunAmountJson<'a> {
    action: &'static str,
    wallet: &'a str,
    amount: u128,
}

#[derive(Serialize)]
struct DryRunNamespaceJson<'a> {
    action: &'static str,
    wallet: &'a str,
    namespace: &'a str,
    registration_fee: u128,
}

#[derive(Serialize)]
struct DryRunPinboardJson<'a> {
    action: &'static str,
    wallet: &'a str,
    file: &'a str,
    user_fee_amount: u128,
}

pub(crate) fn emit_created_wallet(mode: OutputMode, wallet: &Wallet) -> Result<(), EldError> {
    emit(mode, &created_wallet(wallet), &wallet_json(wallet))
}

pub(crate) fn emit_wallet_list(mode: OutputMode, wallets: &[Wallet]) -> Result<(), EldError> {
    let view = WalletListJson {
        wallets: wallets.iter().map(wallet_json).collect(),
    };
    emit(mode, &list_wallets(wallets), &view)
}

pub(crate) fn emit_wallet(mode: OutputMode, wallet: &Wallet) -> Result<(), EldError> {
    emit(mode, &display_wallet(wallet), &wallet_json(wallet))
}

pub(crate) fn emit_removed_wallet(mode: OutputMode, name: &str) -> Result<(), EldError> {
    emit(
        mode,
        &removed_wallet(name),
        &RemovedWalletJson {
            name: name.to_string(),
            removed: true,
        },
    )
}

pub(crate) fn emit_submitted_tx(
    mode: OutputMode,
    kind: &str,
    submitted: &SubmittedTx,
) -> Result<(), EldError> {
    emit(
        mode,
        &submitted_tx(kind, submitted),
        &submitted_tx_json(kind, submitted),
    )
}

pub(crate) fn emit_faucet(mode: OutputMode, body: &str) -> Result<(), EldError> {
    emit(mode, &faucet_ok(body), &faucet_json(body))
}

pub(crate) fn emit_account(mode: OutputMode, account: &Account) -> Result<(), EldError> {
    emit(mode, &self::account(account), account)
}

pub(crate) fn emit_staking_account(
    mode: OutputMode,
    address: &str,
    account: &StakingAccount,
) -> Result<(), EldError> {
    emit(mode, &staking_account(address, account), account)
}

pub(crate) fn emit_abci(mode: OutputMode, info: &AbciInfoWrapper) -> Result<(), EldError> {
    let view = AbciJson {
        app_version: info.app_version,
        version: info.version.clone(),
        last_block_height: info.last_block_height.to_string(),
        last_block_app_hash: info.last_block_app_hash.to_string(),
    };
    emit(mode, &info.to_string(), &view)
}

pub(crate) fn emit_active_validators(
    mode: OutputMode,
    info: Option<&ActiveValidatorsInfo>,
    balances: &[String],
) -> Result<(), EldError> {
    emit(
        mode,
        &active_validators(info, balances),
        &active_validators_json(info, balances),
    )
}

pub(crate) fn emit_epoch(
    mode: OutputMode,
    epoch_info: &EpochInfo,
    validators: &ActiveValidatorsInfo,
) -> Result<(), EldError> {
    emit(
        mode,
        &epoch(epoch_info, validators),
        &EpochJson {
            epoch: epoch_info,
            validators,
        },
    )
}

pub(crate) fn emit_namespace_lookup(
    mode: OutputMode,
    lookup: &NamespaceLookup,
) -> Result<(), EldError> {
    match &lookup.registered {
        Some(resp) => emit(mode, &namespace_lookup(lookup), resp),
        None => emit(
            mode,
            &namespace_lookup(lookup),
            &MissingNamespaceJson {
                registered: false,
                namespace_slug: lookup.canonical_slug.clone(),
            },
        ),
    }
}

pub(crate) fn emit_registered(
    mode: OutputMode,
    resp: &NamespaceRegisteredResponse,
) -> Result<(), EldError> {
    emit(mode, &print_registered(resp), resp)
}

pub(crate) fn emit_pinboard_submit(
    mode: OutputMode,
    resp: &PostMessageSubmitResponse,
) -> Result<(), EldError> {
    emit(mode, &pinboard_submit(resp), resp)
}

pub(crate) fn emit_pinboard_value(
    mode: OutputMode,
    text: &str,
    value: &Value,
) -> Result<(), EldError> {
    emit(mode, text, value)
}

pub(crate) fn emit_cado_list(
    mode: OutputMode,
    search_string: &str,
    paths: &[String],
) -> Result<(), EldError> {
    emit(
        mode,
        &list_cados(search_string, paths),
        &CadoListJson {
            search_string,
            paths,
        },
    )
}

pub(crate) fn emit_cado(mode: OutputMode, path: &str, response: &Value) -> Result<(), EldError> {
    let text = cado(path, response)?;
    emit(mode, &text, response)
}

pub(crate) fn emit_dry_run_transfer(
    mode: OutputMode,
    wallet: &str,
    recipient: &str,
    amount: u128,
) -> Result<(), EldError> {
    emit(
        mode,
        &dry_run_transfer(wallet, recipient, amount),
        &DryRunTransferJson {
            action: "transfer",
            wallet,
            recipient,
            amount,
        },
    )
}

pub(crate) fn emit_dry_run_stake(
    mode: OutputMode,
    wallet: &str,
    amount: u128,
) -> Result<(), EldError> {
    emit(
        mode,
        &dry_run_stake(wallet, amount),
        &DryRunAmountJson {
            action: "stake",
            wallet,
            amount,
        },
    )
}

pub(crate) fn emit_dry_run_unstake(
    mode: OutputMode,
    wallet: &str,
    amount: u128,
) -> Result<(), EldError> {
    emit(
        mode,
        &dry_run_unstake(wallet, amount),
        &DryRunAmountJson {
            action: "unstake",
            wallet,
            amount,
        },
    )
}

pub(crate) fn emit_dry_run_add_namespace(
    mode: OutputMode,
    wallet: &str,
    namespace: &str,
    registration_fee: u128,
) -> Result<(), EldError> {
    emit(
        mode,
        &dry_run_add_namespace(wallet, namespace, registration_fee),
        &DryRunNamespaceJson {
            action: "add-namespace",
            wallet,
            namespace,
            registration_fee,
        },
    )
}

pub(crate) fn emit_dry_run_pinboard_post(
    mode: OutputMode,
    wallet: &str,
    file_path: &str,
    user_fee_amount: u128,
) -> Result<(), EldError> {
    emit(
        mode,
        &dry_run_pinboard_post(wallet, file_path, user_fee_amount),
        &DryRunPinboardJson {
            action: "pinboard post",
            wallet,
            file: file_path,
            user_fee_amount,
        },
    )
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
    text.line(format!("CADO {path}"));
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
            let hash_bytes = json_number_array_as_bytes(hash_array, "cado.hash")?;
            text.blank();
            text.line(format!("Hash:\t\t 0x{}", hex::encode(hash_bytes)));
        }
    }

    if cado_type == "Mutable" {
        if let Some(latest_hash) = cado.get("latest_hash") {
            if let Some(hash_array) = latest_hash.as_array() {
                let hash_bytes = json_number_array_as_bytes(hash_array, "cado.latest_hash")?;
                text.line(format!("Latest Hash:\t 0x{}", hex::encode(hash_bytes)));
            }
        }
    }

    let Some(data) = cado.get("data").and_then(|d| d.as_array()) else {
        return Ok(text.finish());
    };
    let data_bytes = json_number_array_as_bytes(data, "cado.data")?;

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
    } else if let Ok(str_data) = String::from_utf8(data_bytes.clone()) {
        text.line("Data (as string):");
        text.line(format!("* {str_data}"));
    } else {
        text.line("Data (as hex):");
        text.line(format!("* 0x{}", hex::encode(data_bytes)));
    }

    Ok(text.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use eld_client::api::rest::PostMessageSubmitResponse;
    use eld_client::facade::TxHash;
    use eld_common::coin::Coin;
    use eld_common::nonce::Nonce;
    use eld_common::Address;
    const ADDRESS: &str = "0x1234567890abcdef1234567890abcdef12345678";

    fn fixture_wallet() -> Wallet {
        Wallet::from_signing_key("alice".to_string(), SigningKey::from_bytes(&[7; 32]))
    }

    #[test]
    fn wallet_formatter_prints_name_address_and_public_key() {
        let wallet = fixture_wallet();
        let expected = format!(
            "Wallet {{ name: alice, address: {}, public_key: {} }}",
            wallet.address.hex_with_prefix(),
            hex::encode(wallet.public_key)
        );
        assert_eq!(created_wallet(&wallet), expected);
        assert_eq!(display_wallet(&wallet), expected);
        let listed = list_wallets(std::slice::from_ref(&wallet));
        assert!(!expected.contains("private"), "{expected}");
        assert!(!listed.contains("private"), "{listed}");
        assert!(!expected.contains(&hex::encode([7u8; 32])), "{expected}");
        let json = serde_json::to_string(&super::wallet_json(&wallet)).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("name").is_some());
        assert!(!json.contains("private"), "{json}");
        assert!(!json.contains(&hex::encode([7u8; 32])), "{json}");
    }

    #[test]
    fn cado_rejects_out_of_range_byte_instead_of_truncating() {
        let response = serde_json::json!({
            "Mutable": {
                "hash": [0, 255, 256],
                "data": [1, 2, 3]
            }
        });
        let err = cado("/@eld/account/0xabc", &response).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("not a byte"), "{message}");
        assert!(message.contains("256"), "{message}");
    }

    #[test]
    fn cado_keeps_every_hash_byte() {
        let response = serde_json::json!({
            "Immutable": {
                "hash": [0, 255]
            }
        });
        let text = cado("/@eld/other", &response).unwrap();
        assert!(text.contains("0x00ff"), "{text}");
    }

    #[test]
    fn epoch_zero_blocks_per_epoch_does_not_panic() {
        let epoch_info = EpochInfo {
            current_epoch: 1,
            current_block: 1,
            blocks_per_epoch: 0,
            validators_per_epoch: 1,
            blocks_until_next_epoch: 0,
        };
        let validators = ActiveValidatorsInfo {
            validators: Vec::new(),
            total_stake: Coin::zero(),
            current_epoch: 1,
        };
        let text = epoch(&epoch_info, &validators);
        assert!(text.contains("Epoch"), "{text}");
    }

    #[test]
    fn dry_run_text_has_no_tx_hash() {
        let text = dry_run_transfer("alice", ADDRESS, 1000);
        assert!(text.contains("wallet: alice"), "{text}");
        assert!(text.contains(&format!("recipient: {ADDRESS}")), "{text}");
        assert!(text.contains("amount: 1000"), "{text}");
        assert!(!text.contains("tx_hash"), "{text}");
    }

    #[test]
    fn account_formatter_prints_address_balance_and_nonce() {
        let account = Account::new(
            Address::parse_hex_str(ADDRESS).unwrap(),
            Coin::new(1000).unwrap(),
            Nonce::new(3),
        );
        assert_eq!(
            super::account(&account),
            "address: 0x1234567890abcdef1234567890abcdef12345678\nbalance: 0.001000\nnonce: 3"
        );
    }

    #[test]
    fn submitted_tx_formatter_prints_commit_and_events() {
        let tx_hash = TxHash::Sha256([0xab; 32]);
        let submitted = SubmittedTx {
            tx_hash,
            response: serde_json::json!({
                "result": {
                    "deliver_tx": {
                        "events": [{
                            "type": "transfer",
                            "attributes": [{ "key": "c2VuZGVy", "value": "b2s=" }]
                        }]
                    }
                }
            }),
            signed_tx_json: String::new(),
            fee: Coin::new(1000).unwrap(),
            nonce: Nonce::new(7),
        };
        let expected = format!(
            "Transfer transaction committed\ntx_hash: {tx_hash}\nfee: 1000\nnonce: 7\n\nevent_type: transfer\nsender: ok"
        );
        assert_eq!(submitted_tx("Transfer", &submitted), expected);
    }

    #[test]
    fn namespace_formatter_prints_registered_and_missing() {
        let missing = NamespaceLookup {
            canonical_slug: "peter".to_string(),
            registered: None,
        };
        assert_eq!(
            namespace_lookup(&missing),
            "registered: false\nnamespace_slug: peter"
        );

        let registered = NamespaceRegisteredResponse {
            registered: true,
            namespace_slug: "peter".to_string(),
            scope: "@peter".to_string(),
            owner: ADDRESS.to_string(),
            registered_height: 9,
            registry_path: "/ns/peter".to_string(),
        };
        assert_eq!(
            namespace_lookup(&NamespaceLookup {
                canonical_slug: "peter".to_string(),
                registered: Some(registered),
            }),
            "registered: true\nnamespace_slug: peter\nscope: @peter\nowner: 0x1234567890abcdef1234567890abcdef12345678\nregistered_height: 9\nregistry_path: /ns/peter"
        );
    }

    #[test]
    fn pinboard_submit_formatter_prints_accept_fields() {
        let response = PostMessageSubmitResponse::submitted(
            "mid".to_string(),
            "ckey".to_string(),
            "thash".to_string(),
            "validator".to_string(),
            42,
            Some("/@peter/mid".to_string()),
        );
        assert_eq!(
            pinboard_submit(&response),
            "status: submitted\nmessage_id: mid\ncontent_key: ckey\ntx_hash: thash\norigin_validator: validator\nreceived_timestamp: 42\ncontent_path: /@peter/mid"
        );
    }

    #[test]
    fn not_found_error_string() {
        assert_eq!(
            ErrorBuilder::not_found_error("Wallet", "alice").to_string(),
            "Wallet not found: alice"
        );
        assert_eq!(
            ErrorBuilder::not_found_error("Account", ADDRESS).to_string(),
            "Account not found: 0x1234567890abcdef1234567890abcdef12345678"
        );
        assert_eq!(
            ErrorBuilder::not_found_error("CADO", "/eld/account/x").to_string(),
            "CADO not found: /eld/account/x"
        );
    }
}
