use eld_client::facade::ChainClient;
use eld_common::error::EldError;

pub(crate) async fn view_active_validators(
    cli: &ChainClient,
    node_url: &str,
) -> Result<(), EldError> {
    let validators = cli.view_active_validators().await?;
    crate::output::active_validators(cli, node_url, validators).await
}

pub(crate) async fn view_epoch(cli: &ChainClient) -> Result<(), EldError> {
    let (epoch_info, validators) = cli.view_epoch().await?;
    crate::output::epoch(&epoch_info, &validators);
    Ok(())
}
