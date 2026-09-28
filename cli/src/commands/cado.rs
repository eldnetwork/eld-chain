use eld_client::facade::ChainClient;
use eld_common::error::EldError;

pub(crate) async fn get_cado(cli: &ChainClient, path: String) -> Result<(), EldError> {
    let value = cli.get_cado(path.clone()).await?;
    crate::output::cado(&path, &value);
    Ok(())
}

pub(crate) async fn list_cados(cli: &ChainClient, search_string: String) -> Result<(), EldError> {
    let paths = cli.list_cados(search_string.clone()).await?;
    crate::output::list_cados(&search_string, &paths);
    Ok(())
}
