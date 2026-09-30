use eld_client::facade::ChainClient;
use eld_common::error::EldError;

use crate::output::OutputMode;

pub(crate) async fn get_namespace(
    cli: &ChainClient,
    namespace_slug: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let lookup = cli.get_namespace(namespace_slug).await?;
    crate::output::emit_namespace_lookup(mode, &lookup)
}

pub(crate) async fn add_namespace(
    cli: &ChainClient,
    wallet_name: String,
    namespace_slug: String,
    registration_fee: u128,
    mode: OutputMode,
) -> Result<(), EldError> {
    let resp = cli
        .add_namespace(wallet_name, namespace_slug, registration_fee)
        .await?;
    crate::output::emit_registered(mode, &resp)
}
