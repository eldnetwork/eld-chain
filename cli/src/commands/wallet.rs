use eld_client::facade::ChainClient;
use eld_common::error::{EldError, ErrorBuilder};
use std::path::Path;

pub(crate) async fn create_wallet(wallet_path: &Path, name: String) -> Result<(), EldError> {
    let wallet = ChainClient::create_wallet_at(name, wallet_path).await?;
    crate::output::print_result(&crate::output::created_wallet(&wallet));
    Ok(())
}

pub(crate) async fn list_wallets(wallet_path: &Path) -> Result<(), EldError> {
    let wallets = ChainClient::list_wallets_at(wallet_path).await?;
    crate::output::print_result(&crate::output::list_wallets(&wallets));
    Ok(())
}

pub(crate) async fn get_wallet(wallet_path: &Path, name: String) -> Result<(), EldError> {
    let wallet = ChainClient::get_wallet_by_name_at(&name, wallet_path)
        .await?
        .ok_or_else(|| ErrorBuilder::not_found_error("Wallet", &name))?;
    crate::output::print_result(&crate::output::display_wallet(&wallet));
    Ok(())
}

pub(crate) async fn remove_wallet(wallet_path: &Path, name: String) -> Result<(), EldError> {
    let removed = ChainClient::remove_wallet_at(name.clone(), wallet_path).await?;
    if !removed {
        return Err(ErrorBuilder::not_found_error("Wallet", &name));
    }
    crate::output::print_result(&crate::output::removed_wallet(&name));
    Ok(())
}
