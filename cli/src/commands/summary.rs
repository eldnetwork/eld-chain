use eld_client::facade::ChainClient;
use eld_common::error::EldError;

use crate::output::OutputMode;

pub(crate) async fn view_summary(cli: &ChainClient, mode: OutputMode) -> Result<(), EldError> {
    let header = cli.view_summary().await?;
    crate::output::emit_summary(mode, &header)
}
