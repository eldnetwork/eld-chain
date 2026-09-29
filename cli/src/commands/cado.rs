use eld_client::facade::ChainClient;
use eld_common::error::EldError;

use crate::output::OutputMode;

pub(crate) async fn get_cado(
    cli: &ChainClient,
    path: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let value = cli.get_cado(path.clone()).await?;
    crate::output::emit_cado(mode, &path, &value)
}

pub(crate) async fn list_cados(
    cli: &ChainClient,
    search_string: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let paths = cli.list_cados(search_string.clone()).await?;
    crate::output::emit_cado_list(mode, &search_string, &paths)
}
