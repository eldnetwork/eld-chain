mod args;
mod commands;
mod logging;
mod output;

use args::{
    AccountCommand, Arguments, CadoCommand, ChainCommand, NamespaceCommand, PinboardCommand,
    SubCommand, TxCommand, WalletCommand,
};
use clap::Parser;
use eld_client::config::{
    load_client_setup, CONSENSUS_CONFIG_PATH, DEFAULT_CONFIG_PATH, WALLETS_PATH,
};
use eld_client::facade::ChainClient;
use eld_common::error::EldError;
use std::path::{Path, PathBuf};

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

struct CliPaths {
    cli_config: PathBuf,
    wallets: PathBuf,
    consensus_config: PathBuf,
}

fn resolve_paths(args: &Arguments) -> CliPaths {
    let home = &args.home;
    CliPaths {
        cli_config: args
            .config
            .clone()
            .unwrap_or_else(|| home.join(DEFAULT_CONFIG_PATH)),
        wallets: args
            .wallets
            .clone()
            .unwrap_or_else(|| home.join(WALLETS_PATH)),
        consensus_config: args
            .consensus_config
            .clone()
            .unwrap_or_else(|| home.join(CONSENSUS_CONFIG_PATH)),
    }
}

fn command_is_offline(cmd: &SubCommand) -> bool {
    matches!(
        cmd,
        SubCommand::Wallet { .. }
            | SubCommand::CreateWallet(_)
            | SubCommand::ListWallets
            | SubCommand::GetWallet(_)
            | SubCommand::RemoveWallet(_)
    )
}

fn utf8_path(path: &Path) -> Result<&str, EldError> {
    path.to_str().ok_or_else(|| EldError::ValidationError {
        field: "path".to_string(),
        value: path.display().to_string(),
        details: "path must be valid UTF-8".to_string(),
    })
}

fn open_chain_client(paths: &CliPaths) -> Result<ChainClient, EldError> {
    let setup = load_client_setup(
        utf8_path(&paths.cli_config)?,
        utf8_path(&paths.consensus_config)?,
    )?;
    setup.config.get_node_url()?;
    ChainClient::with_wallets(setup.config, setup.fee_config, &paths.wallets)
}

async fn dispatch(args: Arguments) -> Result<(), EldError> {
    let paths = resolve_paths(&args);
    let yes = args.yes;
    let dry_run = args.dry_run;
    if command_is_offline(&args.cmd) {
        return dispatch_offline(&paths.wallets, args.cmd, yes).await;
    }
    let cli = open_chain_client(&paths)?;
    dispatch_online(&cli, args.cmd, dry_run).await
}

async fn dispatch_offline(wallets: &Path, cmd: SubCommand, yes: bool) -> Result<(), EldError> {
    match cmd {
        SubCommand::Wallet {
            cmd: WalletCommand::Create(wallet),
        }
        | SubCommand::CreateWallet(wallet) => {
            commands::wallet::create_wallet(wallets, wallet.name).await
        }
        SubCommand::Wallet {
            cmd: WalletCommand::List,
        }
        | SubCommand::ListWallets => commands::wallet::list_wallets(wallets).await,
        SubCommand::Wallet {
            cmd: WalletCommand::Show(wallet),
        }
        | SubCommand::GetWallet(wallet) => commands::wallet::get_wallet(wallets, wallet.name).await,
        SubCommand::Wallet {
            cmd: WalletCommand::Remove(wallet),
        }
        | SubCommand::RemoveWallet(wallet) => {
            commands::wallet::remove_wallet(wallets, wallet.name, yes).await
        }
        _ => unreachable!("offline dispatch only handles wallet commands"),
    }
}

async fn dispatch_online(
    cli: &ChainClient,
    cmd: SubCommand,
    dry_run: bool,
) -> Result<(), EldError> {
    match cmd {
        SubCommand::Wallet { .. }
        | SubCommand::CreateWallet(_)
        | SubCommand::ListWallets
        | SubCommand::GetWallet(_)
        | SubCommand::RemoveWallet(_) => {
            unreachable!("wallet commands are dispatched offline")
        }
        SubCommand::Tx {
            cmd: TxCommand::Transfer(tx),
        }
        | SubCommand::Transfer(tx) => {
            commands::tx::transfer(
                cli,
                tx.wallet_name,
                tx.recipient.hex_with_prefix(),
                tx.amount.amount(),
                dry_run,
            )
            .await
        }
        SubCommand::Tx {
            cmd: TxCommand::Faucet(faucet),
        }
        | SubCommand::RequestFaucet(faucet) => {
            commands::tx::request_faucet(cli, faucet.address.hex_with_prefix()).await
        }
        SubCommand::Account {
            cmd: AccountCommand::Get(account),
        }
        | SubCommand::GetAccount(account) => {
            commands::account::get_account(cli, account.address.hex_with_prefix()).await
        }
        SubCommand::Account {
            cmd: AccountCommand::StakeGet(account),
        }
        | SubCommand::GetStakeAccount(account) => {
            commands::account::get_stake_account(cli, account.address.hex_with_prefix()).await
        }
        SubCommand::Chain {
            cmd: ChainCommand::AbciInfo,
        }
        | SubCommand::GetAbciInfo => commands::account::get_abci_info(cli).await,
        SubCommand::Tx {
            cmd: TxCommand::Stake(tx),
        }
        | SubCommand::Stake(tx) => {
            commands::tx::stake(cli, tx.wallet_name, tx.amount.amount(), dry_run).await
        }
        SubCommand::Tx {
            cmd: TxCommand::Unstake(tx),
        }
        | SubCommand::Unstake(tx) => {
            commands::tx::unstake(cli, tx.wallet_name, tx.amount.amount(), dry_run).await
        }
        SubCommand::Chain {
            cmd: ChainCommand::Validators,
        }
        | SubCommand::ViewActiveValidators => commands::epoch::view_active_validators(cli).await,
        SubCommand::Chain {
            cmd: ChainCommand::Epoch,
        }
        | SubCommand::ViewEpoch => commands::epoch::view_epoch(cli).await,
        SubCommand::Namespace {
            cmd: NamespaceCommand::Get(namespace),
        }
        | SubCommand::GetNamespace(namespace) => {
            commands::namespace::get_namespace(cli, namespace.namespace_slug).await
        }
        SubCommand::Namespace {
            cmd: NamespaceCommand::Add(namespace),
        }
        | SubCommand::AddNamespace(namespace) => {
            commands::namespace::add_namespace(
                cli,
                namespace.wallet_name,
                namespace.namespace_slug,
                namespace.registration_fee.amount(),
                dry_run,
            )
            .await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::Post(post),
        }
        | SubCommand::PostPinboardMessage(post) => {
            commands::pinboard::post_message(
                cli,
                eld_client::api::rest::PinboardMessageParams {
                    wallet_name: post.wallet_name,
                    file_path: post.file_path,
                    content_type: post.content_type,
                    expires_height: post.expires_height,
                    visibility: post.visibility,
                    topic: post.topic,
                    tags: post.tags,
                    user_fee_amount: post.user_fee_amount.amount(),
                    namespace: post.namespace,
                },
                dry_run,
            )
            .await
        }
        SubCommand::Cado {
            cmd: CadoCommand::Get(cado),
        }
        | SubCommand::GetCado(cado) => commands::cado::get_cado(cli, cado.path).await,
        SubCommand::Cado {
            cmd: CadoCommand::List(cado),
        }
        | SubCommand::ListCados(cado) => commands::cado::list_cados(cli, cado.search_string).await,
        SubCommand::Pinboard {
            cmd: PinboardCommand::Get(post),
        }
        | SubCommand::PinboardGetPost(post) => {
            commands::pinboard::get_post(cli, post.wallet.hex_with_prefix(), post.message_id).await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::ListTag(post),
        }
        | SubCommand::PinboardListByTag(post) => {
            commands::pinboard::list_by_tag(cli, post.tag, post.page, post.page_size).await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::ListWallet(post),
        }
        | SubCommand::PinboardListByWallet(post) => {
            commands::pinboard::list_by_wallet(
                cli,
                post.wallet.hex_with_prefix(),
                post.page,
                post.page_size,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use args::{CreateWalletArgs, GetAccountArgs};

    fn args_with(home: PathBuf, cmd: SubCommand) -> Arguments {
        Arguments {
            cmd,
            home,
            wallets: None,
            consensus_config: None,
            config: None,
            yes: false,
            dry_run: false,
        }
    }

    #[test]
    fn home_sets_default_paths() {
        let paths = resolve_paths(&args_with(
            PathBuf::from("/tmp/eld-home"),
            SubCommand::ListWallets,
        ));
        assert_eq!(
            paths.wallets,
            PathBuf::from("/tmp/eld-home/wallets/wallets.json")
        );
        assert_eq!(
            paths.cli_config,
            PathBuf::from("/tmp/eld-home/config/config.json")
        );
        assert_eq!(
            paths.consensus_config,
            PathBuf::from("/tmp/eld-home/config/consensus_config.json")
        );
    }

    #[tokio::test]
    async fn wallet_list_works_without_node_config() {
        let dir = tempfile::tempdir().unwrap();
        let wallets = dir.path().join("wallets.json");
        let mut create = args_with(
            dir.path().to_path_buf(),
            SubCommand::CreateWallet(CreateWalletArgs {
                name: "alice".to_string(),
            }),
        );
        create.wallets = Some(wallets.clone());
        dispatch(create).await.unwrap();

        let mut list = args_with(
            dir.path().to_path_buf(),
            SubCommand::Wallet {
                cmd: WalletCommand::List,
            },
        );
        list.wallets = Some(wallets);
        dispatch(list).await.unwrap();
    }

    #[tokio::test]
    async fn account_get_fails_when_node_url_missing() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{
                "node_host": "127.0.0.1",
                "node_port": "26657",
                "node_url": "",
                "faucet_host": "127.0.0.1",
                "faucet_port": "8080",
                "faucet_end_point": "/faucet/request",
                "app_port": "9001"
            }"#,
        )
        .unwrap();
        std::fs::write(
            config_dir.join("consensus_config.json"),
            r#"{"chain_id": "eld-testnet-tempelhof"}"#,
        )
        .unwrap();

        let address =
            eld_common::Address::parse_hex_str("0x1234567890abcdef1234567890abcdef12345678")
                .unwrap();
        let err = dispatch(args_with(
            dir.path().to_path_buf(),
            SubCommand::Account {
                cmd: AccountCommand::Get(GetAccountArgs { address }),
            },
        ))
        .await
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("URL"), "{message}");
    }
}
