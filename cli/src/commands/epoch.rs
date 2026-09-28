use eld_client::facade::ChainClient;
use eld_common::error::EldError;

pub(crate) async fn view_active_validators(
    cli: &ChainClient,
    node_url: &str,
) -> Result<(), EldError> {
    let validators = cli.view_active_validators().await?;
    let text = crate::output::active_validators(cli, node_url, validators).await;
    crate::output::print_result(&text);
    Ok(())
}

pub(crate) async fn view_epoch(cli: &ChainClient) -> Result<(), EldError> {
    let (epoch_info, validators) = cli.view_epoch().await?;
    crate::output::print_result(&crate::output::epoch(&epoch_info, &validators));
    Ok(())
}
