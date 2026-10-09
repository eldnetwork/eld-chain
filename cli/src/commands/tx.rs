use eld_client::facade::ChainClient;
use eld_common::error::EldError;

use crate::output::OutputMode;

pub(crate) async fn transfer(
    cli: &ChainClient,
    wallet_name: String,
    recipient: String,
    amount: u128,
    mode: OutputMode,
) -> Result<(), EldError> {
    let submitted = cli.transfer(wallet_name, recipient, amount).await?;
    crate::output::emit_submitted_tx(mode, "Transfer", &submitted)
}

pub(crate) async fn stake(
    cli: &ChainClient,
    wallet_name: String,
    amount: u128,
    mode: OutputMode,
) -> Result<(), EldError> {
    let submitted = cli.stake(wallet_name, amount).await?;
    crate::output::emit_submitted_tx(mode, "Stake", &submitted)
}

pub(crate) async fn unstake(
    cli: &ChainClient,
    wallet_name: String,
    amount: u128,
    mode: OutputMode,
) -> Result<(), EldError> {
    let submitted = cli.unstake(wallet_name, amount).await?;
    crate::output::emit_submitted_tx(mode, "Unstake", &submitted)
}
