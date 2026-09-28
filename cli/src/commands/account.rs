use eld_client::facade::ChainClient;
use eld_common::error::{EldError, ErrorBuilder};

pub(crate) async fn get_account(cli: &ChainClient, address: String) -> Result<(), EldError> {
    let account = cli
        .get_account(address.clone())
        .await?
        .ok_or_else(|| ErrorBuilder::not_found_error("Account", &address))?;
    crate::output::print_result(&crate::output::account(&account));
    Ok(())
}

pub(crate) async fn get_stake_account(cli: &ChainClient, address: String) -> Result<(), EldError> {
    let account = cli
        .get_staking_account(address.clone())
        .await?
        .ok_or_else(|| ErrorBuilder::not_found_error("Staking Account", &address))?;
    crate::output::print_result(&crate::output::staking_account(&address, &account));
    Ok(())
}

pub(crate) async fn get_abci_info(cli: &ChainClient) -> Result<(), EldError> {
    let info = cli.get_abci_info().await?;
    crate::output::print_result(&info.to_string());
    Ok(())
}
