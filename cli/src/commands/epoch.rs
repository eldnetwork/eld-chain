use eld_client::facade::ChainClient;
use eld_common::error::EldError;
use eld_common::validator::ActiveValidatorsInfo;

use crate::output::OutputMode;

pub(crate) async fn view_active_validators(
    cli: &ChainClient,
    mode: OutputMode,
) -> Result<(), EldError> {
    let validators = cli.view_active_validators().await?;
    let balances = validator_balances(cli, validators.as_ref()).await;
    crate::output::emit_active_validators(mode, validators.as_ref(), &balances)
}

pub(crate) async fn view_epoch(cli: &ChainClient, mode: OutputMode) -> Result<(), EldError> {
    let (epoch_info, validators) = cli.view_epoch().await?;
    crate::output::emit_epoch(mode, &epoch_info, &validators)
}

async fn validator_balances(cli: &ChainClient, info: Option<&ActiveValidatorsInfo>) -> Vec<String> {
    let Some(info) = info else {
        return Vec::new();
    };
    let mut balances = Vec::with_capacity(info.validators.len());
    for validator in &info.validators {
        let balance = match cli
            .get_account_by_address(validator.address.to_string())
            .await
        {
            Ok(Some(account)) => account.balance().to_string(),
            Ok(None) => "account not found".to_string(),
            Err(err) => format!("error: {err}"),
        };
        balances.push(balance);
    }
    balances
}
