use eld_client::api::rest::PinboardMessageParams;
use eld_client::facade::ChainClient;
use eld_common::error::EldError;
use eld_common::pinboard::{
    pinboard_eld_post_cado_path, pinboard_tag_query_path, pinboard_wallet_query_path,
};

use crate::output::OutputMode;

pub(crate) async fn post_message(
    cli: &ChainClient,
    params: PinboardMessageParams,
    mode: OutputMode,
) -> Result<(), EldError> {
    let resp = cli.post_pinboard_message(params).await?;
    crate::output::emit_pinboard_submit(mode, &resp)
}

pub(crate) async fn get_post(
    cli: &ChainClient,
    wallet: String,
    message_id: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let path = pinboard_eld_post_cado_path(&wallet, &message_id);
    let value = cli.pinboard_get_post(wallet, message_id).await?;
    let text = crate::output::pinboard_post(&path, &value);
    crate::output::emit_pinboard_value(mode, &text, &value)
}

pub(crate) async fn list_by_tag(
    cli: &ChainClient,
    tag: String,
    page: usize,
    page_size: usize,
    mode: OutputMode,
) -> Result<(), EldError> {
    let path = pinboard_tag_query_path(&tag, page, page_size);
    let value = cli.pinboard_list_by_tag(tag, page, page_size).await?;
    let text = crate::output::pinboard_list(&path, &value);
    crate::output::emit_pinboard_value(mode, &text, &value)
}

pub(crate) async fn list_by_wallet(
    cli: &ChainClient,
    wallet: String,
    page: usize,
    page_size: usize,
    mode: OutputMode,
) -> Result<(), EldError> {
    let path = pinboard_wallet_query_path(&wallet, page, page_size);
    let value = cli.pinboard_list_by_wallet(wallet, page, page_size).await?;
    let text = crate::output::pinboard_list(&path, &value);
    crate::output::emit_pinboard_value(mode, &text, &value)
}
