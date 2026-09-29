use eld_client::facade::ChainClient;
use eld_common::error::EldError;

pub(crate) async fn transfer(
    cli: &ChainClient,
    wallet_name: String,
    recipient: String,
    amount: u128,
    dry_run: bool,
) -> Result<(), EldError> {
    if dry_run {
        crate::output::print_result(&crate::output::dry_run_transfer(
            &wallet_name,
            &recipient,
            amount,
        ));
        return Ok(());
    }
    let submitted = cli.transfer(wallet_name, recipient, amount).await?;
    crate::output::print_result(&crate::output::submitted_tx("Transfer", &submitted));
    Ok(())
}

pub(crate) async fn request_faucet(cli: &ChainClient, address: String) -> Result<(), EldError> {
    let body = cli.request_faucet(address).await?;
    crate::output::print_result(&crate::output::faucet_ok(&body));
    Ok(())
}

pub(crate) async fn stake(
    cli: &ChainClient,
    wallet_name: String,
    amount: u128,
    dry_run: bool,
) -> Result<(), EldError> {
    if dry_run {
        crate::output::print_result(&crate::output::dry_run_stake(&wallet_name, amount));
        return Ok(());
    }
    let submitted = cli.stake(wallet_name, amount).await?;
    crate::output::print_result(&crate::output::submitted_tx("Stake", &submitted));
    Ok(())
}

pub(crate) async fn unstake(
    cli: &ChainClient,
    wallet_name: String,
    amount: u128,
    dry_run: bool,
) -> Result<(), EldError> {
    if dry_run {
        crate::output::print_result(&crate::output::dry_run_unstake(&wallet_name, amount));
        return Ok(());
    }
    let submitted = cli.unstake(wallet_name, amount).await?;
    crate::output::print_result(&crate::output::submitted_tx("Unstake", &submitted));
    Ok(())
}
