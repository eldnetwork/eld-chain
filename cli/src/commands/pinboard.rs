use eld_client::api::rest::PinboardMessageParams;
use eld_client::facade::ChainClient;
use eld_common::error::EldError;

use crate::output::OutputMode;

pub(crate) async fn post_message(
    cli: &ChainClient,
    params: PinboardMessageParams,
    dry_run: bool,
    mode: OutputMode,
) -> Result<(), EldError> {
    if dry_run {
        return crate::output::emit_dry_run_pinboard_post(
            mode,
            &params.wallet_name,
            &params.file_path,
            params.user_fee_amount,
        );
    }
    let resp = cli.post_pinboard_message(params).await?;
    crate::output::emit_pinboard_submit(mode, &resp)
}

pub(crate) async fn get_post(
    cli: &ChainClient,
    wallet: String,
    message_id: String,
    mode: OutputMode,
) -> Result<(), EldError> {
    let path = pinboard_post_path(&wallet, &message_id);
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
    let path = pinboard_tag_path(&tag, page, page_size);
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
    let path = pinboard_wallet_path(&wallet, page, page_size);
    let value = cli.pinboard_list_by_wallet(wallet, page, page_size).await?;
    let text = crate::output::pinboard_list(&path, &value);
    crate::output::emit_pinboard_value(mode, &text, &value)
}

fn pinboard_post_path(wallet: &str, message_id: &str) -> String {
    format!(
        "{}{}/{}/{}",
        eld_common::constants::cado::PATH_PREFIX_PINBOARD,
        eld_common::constants::abci_query::PINBOARD_SEGMENT_POST,
        wallet,
        message_id
    )
}

fn pinboard_tag_path(tag: &str, page: usize, page_size: usize) -> String {
    format!(
        "{}{}/{}/{}/{}",
        eld_common::constants::cado::PATH_PREFIX_PINBOARD,
        eld_common::constants::abci_query::PINBOARD_SEGMENT_TAG,
        tag,
        page,
        page_size
    )
}

fn pinboard_wallet_path(wallet: &str, page: usize, page_size: usize) -> String {
    format!(
        "{}{}/{}/{}/{}",
        eld_common::constants::cado::PATH_PREFIX_PINBOARD,
        eld_common::constants::abci_query::PINBOARD_SEGMENT_WALLET,
        wallet,
        page,
        page_size
    )
}
