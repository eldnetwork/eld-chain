mod args;
mod commands;
mod logging;
mod output;

use args::{Arguments, SubCommand};
use clap::Parser;
use eld_client::config::{get_client_setup_from_arg, WALLETS_PATH};
use eld_client::facade::ChainClient;
use eld_common::error::EldError;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let args = Arguments::parse();
    if let Err(err) = logging::init_default_logging() {
        fail(err);
    }
    if let Err(err) = dispatch(args).await {
        fail(err);
    }
}

fn fail(err: EldError) -> ! {
    eprintln!("{err}");
    std::process::exit(1);
}

async fn dispatch(args: Arguments) -> Result<(), EldError> {
    let setup = get_client_setup_from_arg(&args.config)?;
    setup.config.get_node_url()?;
    let cli = Arc::new(ChainClient::with_wallets(
        setup.config,
        setup.fee_config,
        WALLETS_PATH,
    )?);

    match args.cmd {
        SubCommand::CreateWallet { name } => commands::wallet::create_wallet(&cli, name).await,
        SubCommand::ListWallets {} => commands::wallet::list_wallets(&cli).await,
        SubCommand::GetWallet { name } => commands::wallet::get_wallet(&cli, name).await,
        SubCommand::RemoveWallet { name } => commands::wallet::remove_wallet(&cli, name).await,
        SubCommand::Transfer {
            wallet_name,
            recipient,
            amount,
        } => commands::tx::transfer(&cli, wallet_name, recipient, amount).await,
        SubCommand::RequestFaucet { address } => commands::tx::request_faucet(&cli, address).await,
        SubCommand::GetAbciInfo {} => commands::account::get_abci_info(&cli).await,
        SubCommand::GetAccount { address } => commands::account::get_account(&cli, address).await,
        SubCommand::GetStakeAccount { address } => {
            commands::account::get_stake_account(&cli, address).await
        }
        SubCommand::Stake {
            wallet_name,
            amount,
        } => commands::tx::stake(&cli, wallet_name, amount).await,
        SubCommand::Unstake {
            wallet_name,
            amount,
        } => commands::tx::unstake(&cli, wallet_name, amount).await,
        SubCommand::ViewActiveValidators {} => commands::epoch::view_active_validators(&cli).await,
        SubCommand::ViewEpoch {} => commands::epoch::view_epoch(&cli).await,
        SubCommand::GetNamespace { namespace_slug } => {
            commands::namespace::get_namespace(&cli, namespace_slug).await
        }
        SubCommand::AddNamespace {
            wallet_name,
            namespace_slug,
            registration_fee,
        } => {
            commands::namespace::add_namespace(&cli, wallet_name, namespace_slug, registration_fee)
                .await
        }
        SubCommand::PostPinboardMessage {
            wallet_name,
            file_path,
            content_type,
            expires_height,
            visibility,
            topic,
            tags,
            user_fee_amount,
            namespace,
        } => {
            commands::pinboard::post_message(
                &cli,
                eld_client::api::rest::PinboardMessageParams {
                    wallet_name,
                    file_path,
                    content_type,
                    expires_height,
                    visibility,
                    topic,
                    tags,
                    user_fee_amount,
                    namespace,
                },
            )
            .await
        }
        SubCommand::GetCado { path } => commands::cado::get_cado(&cli, path).await,
        SubCommand::ListCados { search_string } => {
            commands::cado::list_cados(&cli, search_string).await
        }
        SubCommand::PinboardGetPost { wallet, message_id } => {
            commands::pinboard::get_post(&cli, wallet, message_id).await
        }
        SubCommand::PinboardListByTag {
            tag,
            page,
            page_size,
        } => commands::pinboard::list_by_tag(&cli, tag, page, page_size).await,
        SubCommand::PinboardListByWallet {
            wallet,
            page,
            page_size,
        } => commands::pinboard::list_by_wallet(&cli, wallet, page, page_size).await,
    }
}
