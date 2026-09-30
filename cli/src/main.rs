mod args;
mod commands;
mod logging;
mod output;
mod setup;

use args::{
    AccountCommand, Arguments, CadoCommand, ChainCommand, ConfigCommand, NamespaceCommand,
    PinboardCommand, SubCommand, TxCommand, WalletCommand,
};
use clap::Parser;
use eld_client::config::{ClientConfig, FeeConfig};
use eld_client::facade::ChainClient;
use eld_common::error::EldError;
use output::OutputMode;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tracing::Instrument;

#[tokio::main]
async fn main() -> ExitCode {
    let args = match Arguments::try_parse() {
        Ok(args) => args,
        Err(err) => {
            let code = err.exit_code();
            let _ = err.print();
            return u8::try_from(code).map_or(ExitCode::from(2), ExitCode::from);
        }
    };
    if let Err(err) = logging::init_default_logging() {
        eprintln!("{err}");
        return ExitCode::from(1);
    }
    let command = command_name(&args.cmd);
    let span = tracing::info_span!("dispatch", command);
    match dispatch(args).instrument(span).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn command_name(cmd: &SubCommand) -> &'static str {
    match cmd {
        SubCommand::Wallet {
            cmd: WalletCommand::Create(_),
        }
        | SubCommand::CreateWallet(_) => "wallet create",
        SubCommand::Wallet {
            cmd: WalletCommand::List,
        }
        | SubCommand::ListWallets => "wallet list",
        SubCommand::Wallet {
            cmd: WalletCommand::Show(_),
        }
        | SubCommand::GetWallet(_) => "wallet show",
        SubCommand::Wallet {
            cmd: WalletCommand::Remove(_),
        }
        | SubCommand::RemoveWallet(_) => "wallet remove",
        SubCommand::Tx {
            cmd: TxCommand::Transfer(_),
        }
        | SubCommand::Transfer(_) => "tx transfer",
        SubCommand::Tx {
            cmd: TxCommand::Stake(_),
        }
        | SubCommand::Stake(_) => "tx stake",
        SubCommand::Tx {
            cmd: TxCommand::Unstake(_),
        }
        | SubCommand::Unstake(_) => "tx unstake",
        SubCommand::Tx {
            cmd: TxCommand::Faucet(_),
        }
        | SubCommand::RequestFaucet(_) => "tx faucet",
        SubCommand::Account {
            cmd: AccountCommand::Get(_),
        }
        | SubCommand::GetAccount(_) => "account get",
        SubCommand::Account {
            cmd: AccountCommand::StakeGet(_),
        }
        | SubCommand::GetStakeAccount(_) => "account stake-get",
        SubCommand::Chain {
            cmd: ChainCommand::AbciInfo,
        }
        | SubCommand::GetAbciInfo => "chain abci-info",
        SubCommand::Chain {
            cmd: ChainCommand::Epoch,
        }
        | SubCommand::ViewEpoch => "chain epoch",
        SubCommand::Chain {
            cmd: ChainCommand::Validators,
        }
        | SubCommand::ViewActiveValidators => "chain validators",
        SubCommand::Namespace {
            cmd: NamespaceCommand::Get(_),
        }
        | SubCommand::GetNamespace(_) => "namespace get",
        SubCommand::Namespace {
            cmd: NamespaceCommand::Add(_),
        }
        | SubCommand::AddNamespace(_) => "namespace add",
        SubCommand::Pinboard {
            cmd: PinboardCommand::Post(_),
        }
        | SubCommand::PostPinboardMessage(_) => "pinboard post",
        SubCommand::Pinboard {
            cmd: PinboardCommand::Get(_),
        }
        | SubCommand::PinboardGetPost(_) => "pinboard get",
        SubCommand::Pinboard {
            cmd: PinboardCommand::ListTag(_),
        }
        | SubCommand::PinboardListByTag(_) => "pinboard list-tag",
        SubCommand::Pinboard {
            cmd: PinboardCommand::ListWallet(_),
        }
        | SubCommand::PinboardListByWallet(_) => "pinboard list-wallet",
        SubCommand::Cado {
            cmd: CadoCommand::Get(_),
        }
        | SubCommand::GetCado(_) => "cado get",
        SubCommand::Cado {
            cmd: CadoCommand::List(_),
        }
        | SubCommand::ListCados(_) => "cado list",
        SubCommand::Config {
            cmd: ConfigCommand::Node { .. },
        } => "config node",
        SubCommand::Config {
            cmd: ConfigCommand::Faucet { .. },
        } => "config faucet",
        SubCommand::Completions { .. } => "completions",
    }
}

const CONFIG_FILE_NAME: &str = "eld-cli-config.json";
const WALLETS_FILE_NAME: &str = "wallets.json";

struct CliPaths {
    cli_config: PathBuf,
    wallets: PathBuf,
}

fn non_empty_env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

fn user_home() -> Result<PathBuf, EldError> {
    non_empty_env_path("HOME")
        .or_else(|| non_empty_env_path("USERPROFILE"))
        .ok_or_else(|| {
            EldError::make_validation_error(
                "home",
                "missing",
                "No home directory is set. Pass --cli-config and --wallets.",
            )
        })
}

fn platform_config_path(
    home: &Path,
    xdg_config_home: Option<&Path>,
    appdata: Option<&Path>,
) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let _ = (xdg_config_home, appdata);
        home.join("Library/Application Support/eld")
            .join(CONFIG_FILE_NAME)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = appdata;
        let base = xdg_config_home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join(".config"));
        base.join("eld").join(CONFIG_FILE_NAME)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = xdg_config_home;
        let base = appdata
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.to_path_buf());
        base.join("eld").join(CONFIG_FILE_NAME)
    }
}

fn platform_wallets_path(
    home: &Path,
    xdg_data_home: Option<&Path>,
    appdata: Option<&Path>,
) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let _ = (xdg_data_home, appdata);
        home.join("Library/Application Support/eld")
            .join(WALLETS_FILE_NAME)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = appdata;
        let base = xdg_data_home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join(".local/share"));
        base.join("eld").join(WALLETS_FILE_NAME)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = xdg_data_home;
        let base = appdata
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.to_path_buf());
        base.join("eld").join(WALLETS_FILE_NAME)
    }
}

fn default_config_path() -> Result<PathBuf, EldError> {
    let home = user_home()?;
    let xdg = non_empty_env_path("XDG_CONFIG_HOME");
    let appdata = non_empty_env_path("APPDATA");
    Ok(platform_config_path(
        &home,
        xdg.as_deref(),
        appdata.as_deref(),
    ))
}

fn default_wallets_path() -> Result<PathBuf, EldError> {
    let home = user_home()?;
    let xdg = non_empty_env_path("XDG_DATA_HOME");
    let appdata = non_empty_env_path("APPDATA");
    Ok(platform_wallets_path(
        &home,
        xdg.as_deref(),
        appdata.as_deref(),
    ))
}

fn resolve_paths(args: &Arguments) -> Result<CliPaths, EldError> {
    Ok(CliPaths {
        cli_config: match &args.config {
            Some(path) => path.clone(),
            None => default_config_path()?,
        },
        wallets: match &args.wallets {
            Some(path) => path.clone(),
            None => default_wallets_path()?,
        },
    })
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

fn command_is_faucet(cmd: &SubCommand) -> bool {
    matches!(
        cmd,
        SubCommand::Tx {
            cmd: TxCommand::Faucet(_),
        } | SubCommand::RequestFaucet(_)
    )
}

fn command_needs_chain_id(cmd: &SubCommand, dry_run: bool) -> bool {
    if dry_run {
        return false;
    }
    matches!(
        cmd,
        SubCommand::Tx {
            cmd: TxCommand::Transfer(_) | TxCommand::Stake(_) | TxCommand::Unstake(_),
        } | SubCommand::Transfer(_)
            | SubCommand::Stake(_)
            | SubCommand::Unstake(_)
            | SubCommand::Namespace {
                cmd: NamespaceCommand::Add(_),
            }
            | SubCommand::AddNamespace(_)
            | SubCommand::Pinboard {
                cmd: PinboardCommand::Post(_),
            }
            | SubCommand::PostPinboardMessage(_)
    )
}

fn open_chain_client(paths: &CliPaths, config: ClientConfig) -> Result<ChainClient, EldError> {
    ChainClient::with_wallets(config, FeeConfig::default(), &paths.wallets)
}

async fn dispatch(args: Arguments) -> Result<(), EldError> {
    let interactive = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    dispatch_with(args, interactive).await
}

async fn dispatch_with(args: Arguments, interactive: bool) -> Result<(), EldError> {
    tracing::info!("running command");
    if let SubCommand::Completions { shell } = args.cmd {
        args::print_completions(shell)?;
        return Ok(());
    }
    let paths = resolve_paths(&args)?;
    let yes = args.yes;
    let dry_run = args.dry_run;
    let mode = OutputMode::new(args.output);
    if let SubCommand::Config { cmd } = args.cmd {
        return commands::config::run(&paths.cli_config, cmd).await;
    }
    if command_is_offline(&args.cmd) {
        return dispatch_offline(&paths.wallets, args.cmd, yes, mode).await;
    }
    setup::ensure_node(&paths.cli_config, interactive).await?;
    if command_is_faucet(&args.cmd) {
        setup::ensure_faucet(&paths.cli_config, interactive).await?;
    }
    let config = setup::load_client_config(&paths.cli_config)?;
    if command_needs_chain_id(&args.cmd, dry_run) && config.chain_id.trim().is_empty() {
        return Err(setup::chain_id_missing_error());
    }
    let cli = open_chain_client(&paths, config)?;
    dispatch_online(&cli, args.cmd, dry_run, mode).await
}

async fn dispatch_offline(
    wallets: &Path,
    cmd: SubCommand,
    yes: bool,
    mode: OutputMode,
) -> Result<(), EldError> {
    match cmd {
        SubCommand::Wallet {
            cmd: WalletCommand::Create(wallet),
        }
        | SubCommand::CreateWallet(wallet) => {
            commands::wallet::create_wallet(wallets, wallet.name, mode).await
        }
        SubCommand::Wallet {
            cmd: WalletCommand::List,
        }
        | SubCommand::ListWallets => commands::wallet::list_wallets(wallets, mode).await,
        SubCommand::Wallet {
            cmd: WalletCommand::Show(wallet),
        }
        | SubCommand::GetWallet(wallet) => {
            commands::wallet::get_wallet(wallets, wallet.name, mode).await
        }
        SubCommand::Wallet {
            cmd: WalletCommand::Remove(wallet),
        }
        | SubCommand::RemoveWallet(wallet) => {
            commands::wallet::remove_wallet(wallets, wallet.name, yes, mode).await
        }
        _ => unreachable!("offline dispatch only handles wallet commands"),
    }
}

async fn dispatch_online(
    cli: &ChainClient,
    cmd: SubCommand,
    dry_run: bool,
    mode: OutputMode,
) -> Result<(), EldError> {
    match cmd {
        SubCommand::Wallet { .. }
        | SubCommand::CreateWallet(_)
        | SubCommand::ListWallets
        | SubCommand::GetWallet(_)
        | SubCommand::RemoveWallet(_) => {
            unreachable!("wallet commands are dispatched offline")
        }
        SubCommand::Completions { .. } | SubCommand::Config { .. } => {
            unreachable!("completions and config are handled before online dispatch")
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
                mode,
            )
            .await
        }
        SubCommand::Tx {
            cmd: TxCommand::Faucet(faucet),
        }
        | SubCommand::RequestFaucet(faucet) => {
            commands::tx::request_faucet(cli, faucet.address.hex_with_prefix(), mode).await
        }
        SubCommand::Account {
            cmd: AccountCommand::Get(account),
        }
        | SubCommand::GetAccount(account) => {
            commands::account::get_account(cli, account.address.hex_with_prefix(), mode).await
        }
        SubCommand::Account {
            cmd: AccountCommand::StakeGet(account),
        }
        | SubCommand::GetStakeAccount(account) => {
            commands::account::get_stake_account(cli, account.address.hex_with_prefix(), mode).await
        }
        SubCommand::Chain {
            cmd: ChainCommand::AbciInfo,
        }
        | SubCommand::GetAbciInfo => commands::account::get_abci_info(cli, mode).await,
        SubCommand::Tx {
            cmd: TxCommand::Stake(tx),
        }
        | SubCommand::Stake(tx) => {
            commands::tx::stake(cli, tx.wallet_name, tx.amount.amount(), dry_run, mode).await
        }
        SubCommand::Tx {
            cmd: TxCommand::Unstake(tx),
        }
        | SubCommand::Unstake(tx) => {
            commands::tx::unstake(cli, tx.wallet_name, tx.amount.amount(), dry_run, mode).await
        }
        SubCommand::Chain {
            cmd: ChainCommand::Validators,
        }
        | SubCommand::ViewActiveValidators => {
            commands::epoch::view_active_validators(cli, mode).await
        }
        SubCommand::Chain {
            cmd: ChainCommand::Epoch,
        }
        | SubCommand::ViewEpoch => commands::epoch::view_epoch(cli, mode).await,
        SubCommand::Namespace {
            cmd: NamespaceCommand::Get(namespace),
        }
        | SubCommand::GetNamespace(namespace) => {
            commands::namespace::get_namespace(cli, namespace.namespace_slug, mode).await
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
                mode,
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
                mode,
            )
            .await
        }
        SubCommand::Cado {
            cmd: CadoCommand::Get(cado),
        }
        | SubCommand::GetCado(cado) => commands::cado::get_cado(cli, cado.path, mode).await,
        SubCommand::Cado {
            cmd: CadoCommand::List(cado),
        }
        | SubCommand::ListCados(cado) => {
            commands::cado::list_cados(cli, cado.search_string, mode).await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::Get(post),
        }
        | SubCommand::PinboardGetPost(post) => {
            commands::pinboard::get_post(cli, post.wallet.hex_with_prefix(), post.message_id, mode)
                .await
        }
        SubCommand::Pinboard {
            cmd: PinboardCommand::ListTag(post),
        }
        | SubCommand::PinboardListByTag(post) => {
            commands::pinboard::list_by_tag(cli, post.tag, post.page, post.page_size, mode).await
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
                mode,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use args::{CreateWalletArgs, GetAccountArgs};

    fn args_with(cmd: SubCommand) -> Arguments {
        Arguments {
            cmd,
            wallets: None,
            config: None,
            yes: false,
            dry_run: false,
            output: args::OutputFormat::Text,
        }
    }

    fn args_in(dir: &Path, cmd: SubCommand) -> Arguments {
        let mut args = args_with(cmd);
        args.config = Some(dir.join("config").join("eld-cli-config.json"));
        args.wallets = Some(dir.join("wallets").join("wallets.json"));
        args
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_defaults_use_application_support_and_ignore_xdg() {
        let home = Path::new("/Users/eld");
        let xdg = Path::new("/xdg");
        assert_eq!(
            platform_config_path(home, Some(xdg), None),
            home.join("Library/Application Support/eld/eld-cli-config.json")
        );
        assert_eq!(
            platform_wallets_path(home, Some(xdg), None),
            home.join("Library/Application Support/eld/wallets.json")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_defaults_use_xdg_or_home_dirs() {
        let home = Path::new("/home/eld");
        assert_eq!(
            platform_config_path(home, None, None),
            home.join(".config/eld/eld-cli-config.json")
        );
        assert_eq!(
            platform_wallets_path(home, None, None),
            home.join(".local/share/eld/wallets.json")
        );
        let xdg_config = Path::new("/tmp/xdg-config");
        let xdg_data = Path::new("/tmp/xdg-data");
        assert_eq!(
            platform_config_path(home, Some(xdg_config), None),
            xdg_config.join("eld/eld-cli-config.json")
        );
        assert_eq!(
            platform_wallets_path(home, Some(xdg_data), None),
            xdg_data.join("eld/wallets.json")
        );
    }

    #[test]
    fn explicit_paths_replace_platform_defaults() {
        let mut args = args_with(SubCommand::ListWallets);
        args.config = Some(PathBuf::from("/tmp/eld-cli-config.json"));
        args.wallets = Some(PathBuf::from("/tmp/wallets.json"));
        let paths = resolve_paths(&args).unwrap();
        assert_eq!(paths.cli_config, PathBuf::from("/tmp/eld-cli-config.json"));
        assert_eq!(paths.wallets, PathBuf::from("/tmp/wallets.json"));
    }

    #[tokio::test]
    async fn wallet_list_works_without_node_config() {
        let dir = tempfile::tempdir().unwrap();
        let wallets = dir.path().join("wallets.json");
        let mut create = args_with(SubCommand::CreateWallet(CreateWalletArgs {
            name: "alice".to_string(),
        }));
        create.wallets = Some(wallets.clone());
        dispatch(create).await.unwrap();

        let mut list = args_with(SubCommand::Wallet {
            cmd: WalletCommand::List,
        });
        list.wallets = Some(wallets);
        dispatch(list).await.unwrap();
    }

    #[tokio::test]
    async fn account_get_fails_when_node_url_missing() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("eld-cli-config.json"),
            r#"{
                "node_host": "127.0.0.1",
                "node_port": "26657",
                "node_url": "",
                "faucet_host": "127.0.0.1",
                "faucet_port": "8080",
                "faucet_end_point": "/faucet/request",
                "app_port": "9001",
                "chain_id": "eld-testnet-tempelhof"
            }"#,
        )
        .unwrap();

        let address =
            eld_common::Address::parse_hex_str("0x1234567890abcdef1234567890abcdef12345678")
                .unwrap();
        let err = dispatch(args_in(
            dir.path(),
            SubCommand::Account {
                cmd: AccountCommand::Get(GetAccountArgs { address }),
            },
        ))
        .await
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("URL"), "{message}");
    }

    #[tokio::test]
    async fn node_command_without_config_names_config_node_when_not_a_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let err = dispatch_with(
            args_in(
                dir.path(),
                SubCommand::Chain {
                    cmd: ChainCommand::Epoch,
                },
            ),
            false,
        )
        .await
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("config node"), "{message}");
        assert!(!dir.path().join("config/eld-cli-config.json").exists());
    }

    #[tokio::test]
    async fn faucet_without_address_names_config_faucet_when_not_a_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("eld-cli-config.json"),
            r#"{
                "node_host": "127.0.0.1",
                "node_port": "26657",
                "app_port": "9001",
                "chain_id": "eld-dev"
            }"#,
        )
        .unwrap();
        let address =
            eld_common::Address::parse_hex_str("0x1234567890abcdef1234567890abcdef12345678")
                .unwrap();
        let err = dispatch_with(
            args_in(
                dir.path(),
                SubCommand::Tx {
                    cmd: TxCommand::Faucet(args::FaucetArgs { address }),
                },
            ),
            false,
        )
        .await
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("config faucet"), "{message}");
    }

    #[tokio::test]
    async fn signing_without_chain_id_names_config_node() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("eld-cli-config.json"),
            r#"{
                "node_host": "127.0.0.1",
                "node_port": "26657",
                "app_port": "9001"
            }"#,
        )
        .unwrap();
        let address =
            eld_common::Address::parse_hex_str("0x1234567890abcdef1234567890abcdef12345678")
                .unwrap();
        let err = dispatch_with(
            args_in(
                dir.path(),
                SubCommand::Tx {
                    cmd: TxCommand::Transfer(args::TransferArgs {
                        wallet_name: "alice".to_string(),
                        recipient: address,
                        amount: eld_common::coin::Coin::new(1).unwrap(),
                    }),
                },
            ),
            false,
        )
        .await
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("config node"), "{message}");
        assert!(message.contains("Chain ID"), "{message}");
    }
}
