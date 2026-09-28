use eld_client::facade::ChainClient;
use eld_common::error::{EldError, ErrorBuilder};

pub(crate) async fn create_wallet(cli: &ChainClient, name: String) -> Result<(), EldError> {
    let wallet = cli.create_wallet(name).await?;
    crate::output::print_result(&crate::output::created_wallet(&wallet));
    Ok(())
}

pub(crate) async fn list_wallets(cli: &ChainClient) -> Result<(), EldError> {
    let wallets = cli.list_wallets().await?;
    crate::output::print_result(&crate::output::list_wallets(&wallets));
    Ok(())
}

pub(crate) async fn get_wallet(cli: &ChainClient, name: String) -> Result<(), EldError> {
    let wallet = cli
        .get_wallet_by_name(name.clone())
        .await?
        .ok_or_else(|| ErrorBuilder::not_found_error("Wallet", &name))?;
    crate::output::print_result(&crate::output::display_wallet(&wallet));
    Ok(())
}

pub(crate) async fn remove_wallet(cli: &ChainClient, name: String) -> Result<(), EldError> {
    let removed = cli.remove_wallet(name.clone()).await?;
    if !removed {
        return Err(ErrorBuilder::not_found_error("Wallet", &name));
    }
    crate::output::print_result(&crate::output::removed_wallet(&name));
    Ok(())
}
