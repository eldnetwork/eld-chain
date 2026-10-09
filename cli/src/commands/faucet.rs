use eld_client::facade::ChainClient;
use eld_common::error::EldError;
use eld_common::Address;
use std::path::Path;

use crate::output::OutputMode;

pub(crate) async fn request_faucet(
    cli: &ChainClient,
    address: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let body = cli.request_faucet(address).await?;
    crate::output::emit_faucet(mode, &body)
}

/// Hex address first; otherwise a name in the local wallet file.
pub(crate) async fn resolve_recipient(raw: &str, wallets: &Path) -> Result<Address, EldError> {
    if let Ok(address) = Address::parse_hex_str(raw) {
        return Ok(address);
    }
    match ChainClient::get_wallet_by_name_at(raw, wallets).await? {
        Some(wallet) => Ok(wallet.address),
        None => Err(EldError::make_validation_error(
            "recipient",
            raw,
            format!("'{raw}' is not a hex address and not the name of a local wallet"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::wallet::create_wallet;

    const ADDRESS: &str = "0x1234567890abcdef1234567890abcdef12345678";

    #[tokio::test]
    async fn resolve_accepts_hex_without_wallet_file() {
        let dir = tempfile::tempdir().unwrap();
        let wallets = dir.path().join("wallets.json");
        let address = resolve_recipient(ADDRESS, &wallets).await.unwrap();
        assert_eq!(address.hex_with_prefix(), ADDRESS);
    }

    #[tokio::test]
    async fn resolve_accepts_local_wallet_name() {
        let dir = tempfile::tempdir().unwrap();
        let wallets = dir.path().join("wallets.json");
        create_wallet(&wallets, "alice".to_string(), OutputMode::text())
            .await
            .unwrap();
        let listed = ChainClient::list_wallets_at(&wallets).await.unwrap();
        let expected = listed[0].address.hex_with_prefix();
        let address = resolve_recipient("alice", &wallets).await.unwrap();
        assert_eq!(address.hex_with_prefix(), expected);
    }

    #[tokio::test]
    async fn resolve_rejects_unknown_name() {
        let dir = tempfile::tempdir().unwrap();
        let wallets = dir.path().join("wallets.json");
        let err = resolve_recipient("nobody", &wallets).await.unwrap_err();
        let message = err.to_string();
        assert!(message.contains("nobody"), "{message}");
        assert!(message.contains("hex address"), "{message}");
        assert!(message.contains("local wallet"), "{message}");
    }
}
