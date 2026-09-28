mod args;
mod commands;
mod logging;
mod output;

use args::{
    AccountCommand, Arguments, CadoCommand, ChainCommand, NamespaceCommand, PinboardCommand,
    SubCommand, TxCommand, WalletCommand,
};
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
        SubCommand::Wallet {
            cmd: WalletCommand::Create(wallet),
        }
        | SubCommand::CreateWallet(wallet) => {
            commands::wallet::create_wallet(&cli, wallet.name).await
        }
        SubCommand::Wallet {
            cmd: WalletCommand::List,
        }
        | SubCommand::ListWallets => commands::wallet::list_wallets(&cli).await,
        SubCommand::Wallet {
            cmd: WalletCommand::Show(wallet),
        }
        | SubCommand::GetWallet(wallet) => commands::wallet::get_wallet(&cli, wallet.name).await,
        SubCommand::Wallet {
            cmd: WalletCommand::Remove(wallet),
        }
        | SubCommand::RemoveWallet(wallet) => {
            commands::wallet::remove_wallet(&cli, wallet.name).await
        }
        SubCommand::Tx {
            cmd: TxCommand::Transfer(tx),
        }
        | SubCommand::Transfer(tx) => {
            commands::tx::transfer(&cli, tx.wallet_name, tx.recipient, tx.amount).await
        }
        SubCommand::Tx {
            cmd: TxCommand::Faucet(faucet),
        }
        | SubCommand::RequestFaucet(faucet) => {
            commands::tx::request_faucet(&cli, faucet.address).await
        }
        SubCommand::Account {
            cmd: AccountCommand::Get(account),
        }
        | SubCommand::GetAccount(account) => {
            commands::account::get_account(&cli, account.address).await
        }
        SubCommand::Account {
            cmd: AccountCommand::StakeGet(account),
        }
        | SubCommand::GetStakeAccount(account) => {
            commands::account::get_stake_account(&cli, account.address).await
        }
        SubCommand::Chain {
            cmd: ChainCommand::AbciInfo,
        }
        | SubCommand::GetAbciInfo => commands::account::get_abci_info(&cli).await,
        SubCommand::Tx {
            cmd: TxCommand::Stake(tx),
        }
        | SubCommand::Stake(tx) => commands::tx::stake(&cli, tx.wallet_name, tx.amount).await,
        SubCommand::Tx {
            cmd: TxCommand::Unstake(tx),
        }
        | SubCommand::Unstake(tx) => commands::tx::unstake(&cli, tx.wallet_name, tx.amount).await,
        SubCommand::Chain {
            cmd: ChainCommand::Validators,
        }
        | SubCommand::ViewActiveValidators => commands::epoch::view_active_validators(&cli).await,
        SubCommand::Chain {
            cmd: ChainCommand::Epoch,
        }
        | SubCommand::ViewEpoch => commands::epoch::view_epoch(&cli).await,
        SubCommand::Namespace {
            cmd: NamespaceCommand::Get(namespace),
        }
        | SubCommand::GetNamespace(namespace) => {
            commands::namespace::get_namespace(&cli, namespace.namespace_slug).await
        }
        SubCommand::Namespace {
            cmd: NamespaceCommand::Add(namespace),
        }
        | SubCommand::AddNamespace(namespace) => {
            commands::namespace::add_namespace(
                &cli,
                namespace.wallet_name,
                namespace.namespace_slug,
                namespace.registration_fee,
            )
            .await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::Post(post),
        }
        | SubCommand::PostPinboardMessage(post) => {
            commands::pinboard::post_message(
                &cli,
                eld_client::api::rest::PinboardMessageParams {
                    wallet_name: post.wallet_name,
                    file_path: post.file_path,
                    content_type: post.content_type,
                    expires_height: post.expires_height,
                    visibility: post.visibility,
                    topic: post.topic,
                    tags: post.tags,
                    user_fee_amount: post.user_fee_amount,
                    namespace: post.namespace,
                },
            )
            .await
        }
        SubCommand::Cado {
            cmd: CadoCommand::Get(cado),
        }
        | SubCommand::GetCado(cado) => commands::cado::get_cado(&cli, cado.path).await,
        SubCommand::Cado {
            cmd: CadoCommand::List(cado),
        }
        | SubCommand::ListCados(cado) => commands::cado::list_cados(&cli, cado.search_string).await,
        SubCommand::Pinboard {
            cmd: PinboardCommand::Get(post),
        }
        | SubCommand::PinboardGetPost(post) => {
            commands::pinboard::get_post(&cli, post.wallet, post.message_id).await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::ListTag(post),
        }
        | SubCommand::PinboardListByTag(post) => {
            commands::pinboard::list_by_tag(&cli, post.tag, post.page, post.page_size).await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::ListWallet(post),
        }
        | SubCommand::PinboardListByWallet(post) => {
            commands::pinboard::list_by_wallet(&cli, post.wallet, post.page, post.page_size).await
        }
    }
}
