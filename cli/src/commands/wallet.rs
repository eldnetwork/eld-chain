use eld_client::facade::ChainClient;
use eld_common::error::{EldError, ErrorBuilder};
use std::io::IsTerminal;
use std::path::Path;

pub(crate) async fn create_wallet(wallet_path: &Path, name: String) -> Result<(), EldError> {
    crate::output::warn_unencrypted_wallets();
    let wallet = ChainClient::create_wallet_at(name, wallet_path).await?;
    crate::output::print_result(&crate::output::created_wallet(&wallet));
    Ok(())
}

pub(crate) async fn list_wallets(wallet_path: &Path) -> Result<(), EldError> {
    crate::output::warn_unencrypted_wallets();
    let wallets = ChainClient::list_wallets_at(wallet_path).await?;
    crate::output::print_result(&crate::output::list_wallets(&wallets));
    Ok(())
}

pub(crate) async fn get_wallet(wallet_path: &Path, name: String) -> Result<(), EldError> {
    crate::output::warn_unencrypted_wallets();
    let wallet = ChainClient::get_wallet_by_name_at(&name, wallet_path)
        .await?
        .ok_or_else(|| ErrorBuilder::not_found_error("Wallet", &name))?;
    crate::output::print_result(&crate::output::display_wallet(&wallet));
    Ok(())
}

pub(crate) async fn remove_wallet(
    wallet_path: &Path,
    name: String,
    yes: bool,
) -> Result<(), EldError> {
    crate::output::warn_unencrypted_wallets();
    let interactive = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    remove_wallet_confirmed(wallet_path, name, yes, interactive).await
}

pub(crate) async fn remove_wallet_confirmed(
    wallet_path: &Path,
    name: String,
    yes: bool,
    interactive: bool,
) -> Result<(), EldError> {
    confirm_wallet_remove(&name, yes, interactive)?;
    let removed = ChainClient::remove_wallet_at(name.clone(), wallet_path).await?;
    if !removed {
        return Err(ErrorBuilder::not_found_error("Wallet", &name));
    }
    crate::output::print_result(&crate::output::removed_wallet(&name));
    Ok(())
}

pub(crate) fn confirm_wallet_remove(
    name: &str,
    yes: bool,
    interactive: bool,
) -> Result<(), EldError> {
    if yes {
        return Ok(());
    }
    if !interactive {
        return Err(ErrorBuilder::validation_error(
            "yes",
            name,
            "refusing to remove wallet without --yes when not running in a terminal",
        ));
    }
    eprintln!("Remove wallet '{name}' and its private key? [y/N]");
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).map_err(|err| {
        ErrorBuilder::validation_error("yes", name, &format!("failed to read confirmation: {err}"))
    })?;
    if !matches!(answer.trim(), "y" | "Y") {
        return Err(ErrorBuilder::validation_error(
            "yes",
            name,
            "wallet remove cancelled",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_without_yes_in_non_tty_returns_error() {
        let err = confirm_wallet_remove("alice", false, false).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("--yes"), "{message}");
        assert!(message.contains("alice"), "{message}");
    }

    #[tokio::test]
    async fn remove_without_yes_in_non_tty_does_not_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallets.json");
        create_wallet(&path, "alice".to_string()).await.unwrap();

        let err = remove_wallet_confirmed(&path, "alice".to_string(), false, false)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("--yes"), "{err}");

        let wallets = ChainClient::list_wallets_at(&path).await.unwrap();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].name, "alice");
    }
}
