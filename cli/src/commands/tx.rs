use eld_client::facade::ChainClient;
use eld_common::error::EldError;

pub(crate) async fn transfer(
    cli: &ChainClient,
    wallet_name: String,
    recipient: String,
    amount: u128,
) -> Result<(), EldError> {
    let submitted = cli.transfer(wallet_name, recipient, amount).await?;
    crate::output::print_result(&crate::output::submitted_tx("Transfer", &submitted));
    Ok(())
}

pub(crate) async fn request_faucet(cli: &ChainClient, address: String) -> Result<(), EldError> {
    let body = cli.request_faucet(address).await?;
    crate::output::print_result(&crate::output::faucet_ok(&body));
    Ok(())
}

pub(crate) async fn list_all_transactions(cli: &ChainClient) -> Result<(), EldError> {
    let txs = cli.list_all_transactions().await?;
    crate::output::print_result(&crate::output::all_transactions(&txs));
    Ok(())
}

pub(crate) async fn stake(
    cli: &ChainClient,
    wallet_name: String,
    amount: u128,
) -> Result<(), EldError> {
    let submitted = cli.stake(wallet_name, amount).await?;
    let body = crate::output::submitted_tx("Stake", &submitted);
    crate::output::print_result(&format!("next_nonce: {}\n{body}", submitted.nonce));
    Ok(())
}

pub(crate) async fn unstake(
    cli: &ChainClient,
    wallet_name: String,
    amount: u128,
) -> Result<(), EldError> {
    let submitted = cli.unstake(wallet_name, amount).await?;
    crate::output::print_result(&crate::output::submitted_tx("Unstake", &submitted));
    Ok(())
}
