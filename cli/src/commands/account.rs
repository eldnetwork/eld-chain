use eld_client::facade::ChainClient;
use eld_common::error::{EldError, ErrorBuilder};

use crate::output::OutputMode;

pub(crate) async fn get_account(
    cli: &ChainClient,
    address: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let account = cli
        .get_account(address.clone())
        .await?
        .ok_or_else(|| ErrorBuilder::not_found_error("Account", &address))?;
    crate::output::emit_account(mode, &account)
}

pub(crate) async fn get_stake_account(
    cli: &ChainClient,
    address: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let account = cli
        .get_staking_account(address.clone())
        .await?
        .ok_or_else(|| ErrorBuilder::not_found_error("Staking Account", &address))?;
    crate::output::emit_staking_account(mode, &address, &account)
}

pub(crate) async fn get_abci_info(cli: &ChainClient, mode: OutputMode) -> Result<(), EldError> {
    let info = cli.get_abci_info().await?;
    crate::output::emit_abci(mode, &info)
}
