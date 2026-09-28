use eld_client::facade::ChainClient;
use eld_common::error::{EldError, ErrorBuilder};
use tracing::info;

pub(crate) async fn get_account(cli: &ChainClient, address: String) -> Result<(), EldError> {
    let account = cli.get_account(address.clone()).await?;
    crate::output::account(&address, account.as_ref());
    Ok(())
}

pub(crate) async fn get_stake_account(cli: &ChainClient, address: String) -> Result<(), EldError> {
    match cli.get_staking_account(address.clone()).await? {
        Some(account) => {
            crate::output::staking_account(&address, &account);
            Ok(())
        }
        None => {
            warn_staking_missing(&address);
            Err(ErrorBuilder::not_found_error("Staking Account", &address))
        }
    }
}

pub(crate) async fn get_abci_info(cli: &ChainClient) -> Result<(), EldError> {
    let info = cli.get_abci_info().await?;
    info!("{}", info);
    Ok(())
}

fn warn_staking_missing(address: &str) {
    tracing::warn!(address = %address, "Staking account not found");
}
