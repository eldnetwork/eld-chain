//! Current chain summary header.

use super::ChainClient;
use crate::api::abci::SummaryHeader;
use eld_common::error::EldError;

pub(crate) async fn view_summary(client: &ChainClient) -> Result<SummaryHeader, EldError> {
    crate::api::abci::query::get_summary(&client.config).await
}
