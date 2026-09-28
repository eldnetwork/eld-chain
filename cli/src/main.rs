use clap::{Parser, Subcommand};
use eld_client::config::{get_client_setup_from_arg, WALLETS_PATH};
use eld_client::facade::ChainClient;
use eld_common::error::{EldError, ErrorBuilder};
use std::str;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{fmt::time::UtcTime, prelude::*, EnvFilter};

mod display;

fn init_default_logging() -> Result<(), EldError> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("eld=info"));

    let subscriber = tracing_subscriber::registry().with(env_filter).with(
        tracing_subscriber::fmt::layer()
            .with_timer(UtcTime::rfc_3339())
            .with_target(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(false)
            .with_line_number(false),
    );

    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        EldError::InitializationError {
            component: "logging system".to_string(),
            details: format!("Failed to set global default subscriber: {e}"),
        }
    })?;

    tracing::info!("Logging system initialized");
    Ok(())
}

#[derive(Parser, Debug)]
#[command(author = "ELD LABS", version, about = "Eld Cli")]
pub struct Arguments {
    #[command(subcommand)]
    cmd: SubCommand,

    /// Config file to use (default: config/config.json)
    #[arg(long = "cli-config", default_value = "config/config.json")]
    config: String,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    /// Create Wallet (A Wallet contains a keypair for signing and verifying transactions)
    CreateWallet {
        #[arg()]
        // the name of the wallet
        name: String,
    },
    /// List wallets stored locally
    ListWallets {},
    /// Fetch wallet by name
    GetWallet {
        // Wallet name
        name: String,
    },
    /// Remove a wallet.  Warning: removes private key
    RemoveWallet {
        #[arg()]
        // name of wallet to be removed
        name: String,
    },
    /// Do a Transfer
    Transfer {
        #[arg()]
        // wallet which will sign and be debited
        wallet_name: String,
        #[arg()]
        // receiver of amount
        recipient: String,
        #[arg()]
        // amount to be transferred
        amount: u128,
    },
    /// Request tokens from the faucet
    /// Example: cargo run request-faucet 0x1234567890abcdef1234567890abcdef12345678
    RequestFaucet {
        #[arg()]
        // address to receive tokens
        address: String,
    },
    /// Get Account (Account is a read only view of a wallet, fetched from a node)
    GetAccount {
        #[arg()]
        // address
        address: String,
    },
    GetStakeAccount {
        #[arg()]
        // address
        address: String,
    },
    /// Get ABCI Info
    GetAbciInfo {},
    /// Lists all transactions for the chain. For testing. Will be removed.
    // todo: remove
    ListAllTransactions {},
    /// Stake tokens
    Stake {
        #[arg()]
        // wallet which will sign and be debited
        wallet_name: String,
        #[arg()]
        // amount to stake
        amount: u128,
    },

    /// Unstake tokens
    Unstake {
        #[arg()]
        // wallet which will sign and receive funds
        wallet_name: String,
        #[arg()]
        // amount to unstake
        amount: u128,
    },
    ViewActiveValidators {},
    /// View comprehensive epoch information including active validators
    ViewEpoch {},
    /// Look up a namespace slug in the on-chain registry (app REST API).
    /// Example: cargo run -- get-namespace peter
    GetNamespace {
        #[arg()]
        namespace_slug: String,
    },
    /// Register a custom namespace scope on-chain (`AddNamespace` tx), then poll until visible via REST.
    /// Example: cargo run -- add-namespace wallet1 peter
    AddNamespace {
        #[arg()]
        wallet_name: String,
        #[arg()]
        namespace_slug: String,
        /// Fee paid to register the namespace (must be greater than zero).
        #[arg(long, default_value_t = 1)]
        registration_fee: u128,
    },
    /// Post a pinboard message (user-signed blob + node broadcasts PostMessage tx)
    PostPinboardMessage {
        #[arg()]
        wallet_name: String,
        #[arg()]
        file_path: String,
        /// MIME type for the message body (`text/plain`, `application/json`, or `image/png`).
        #[arg(long = "content-type")]
        content_type: String,
        /// Post lifetime in blocks (TTL). Pinboard queries hide the body after `committed_height + ttl`.
        /// If `ttl` is `0`, it defaults to `1000` blocks.
        #[arg(long, default_value_t = 1000)]
        expires_height: u64,
        #[arg(long, default_value = "Public")]
        visibility: String,
        #[arg(long)]
        topic: Option<String>,
        /// Filter tags (max 4, each max 64 UTF-8 bytes); repeat flag. Address-shaped tags are normalized to 0x + lowercase hex.
        #[arg(long = "tag", action = clap::ArgAction::Append)]
        tags: Vec<String>,
        #[arg(long, default_value_t = 1000)]
        user_fee_amount: u128,
        /// Custom namespace slug (letter-only; must be registered and owned by the posting wallet).
        #[arg(long)]
        namespace: Option<String>,
    },
    /// Query a CADO by its path
    GetCado {
        #[arg()]
        // CADO path (ELD root prefixes: `eld_common::constants::cado::PATH_PREFIX_*` + segments)
        path: String,
    },
    /// List CADO paths matching a search string
    ListCados {
        #[arg()]
        // search string (prefix under `eld_common::constants::cado`, e.g. `PATH_PREFIX_ELD_ROOT_SCOPE`)
        search_string: String,
    },
    /// Pinboard: get a single post by wallet + message_id
    PinboardGetPost {
        #[arg()]
        wallet: String,
        #[arg()]
        message_id: String,
    },
    /// Pinboard: list posts by tag (paged)
    PinboardListByTag {
        #[arg()]
        tag: String,
        #[arg(long, default_value_t = 0)]
        page: usize,
        #[arg(long, default_value_t = 100)]
        page_size: usize,
    },
    /// Pinboard: list posts by wallet (paged)
    PinboardListByWallet {
        #[arg()]
        wallet: String,
        #[arg(long, default_value_t = 0)]
        page: usize,
        #[arg(long, default_value_t = 100)]
        page_size: usize,
    },
}

#[tokio::main]
pub async fn main() -> Result<(), EldError> {
    let args = Arguments::parse();
    let setup = get_client_setup_from_arg(&args.config)?;
    let node_url = setup.config.get_node_url()?;
    let cli = Arc::new(ChainClient::with_wallets(
        setup.config,
        setup.fee_config,
        WALLETS_PATH,
    )?);

    init_default_logging()?;

    print_name();

    let result: Result<(), EldError> = match args.cmd {
        SubCommand::CreateWallet { name } => {
            let wallet = cli.create_wallet(name).await?;
            display::created_wallet(&wallet);
            Ok(())
        }
        SubCommand::ListWallets {} => {
            let wallets = cli.list_wallets().await?;
            display::list_wallets(&wallets);
            Ok(())
        }
        SubCommand::GetWallet { name } => {
            let wallet = cli.get_wallet_by_name(name.clone()).await?;
            display::display_wallet(&name, wallet.as_ref());
            Ok(())
        }
        SubCommand::RemoveWallet { name } => {
            let removed = cli.remove_wallet(name.clone()).await?;
            display::removed_wallet(&name, removed);
            Ok(())
        }
        SubCommand::Transfer {
            wallet_name,
            recipient,
            amount,
        } => {
            let submitted = cli.transfer(wallet_name, recipient, amount).await?;
            display::submitted_tx("Transfer", &submitted);
            Ok(())
        }
        SubCommand::RequestFaucet { address } => {
            let body = cli.request_faucet(address).await?;
            display::faucet_ok(&body);
            Ok(())
        }
        SubCommand::GetAbciInfo {} => {
            let info = cli.get_abci_info().await?;
            info!("{}", info);
            Ok(())
        }
        SubCommand::GetAccount { address } => {
            let account = cli.get_account(address.clone()).await?;
            display::account(&address, account.as_ref());
            Ok(())
        }
        SubCommand::GetStakeAccount { address } => {
            match cli.get_staking_account(address.clone()).await? {
                Some(account) => {
                    display::staking_account(&address, &account);
                    Ok(())
                }
                None => {
                    warn_staking_missing(&address);
                    Err(ErrorBuilder::not_found_error("Staking Account", &address))
                }
            }
        }
        SubCommand::ListAllTransactions {} => {
            let txs = cli.list_all_transactions().await?;
            display::all_transactions(&txs);
            Ok(())
        }
        SubCommand::Stake {
            wallet_name,
            amount,
        } => {
            info!("Stake");
            let submitted = cli.stake(wallet_name, amount).await?;
            info!("next_nonce: {}", submitted.nonce);
            display::submitted_tx("Stake", &submitted);
            Ok(())
        }
        SubCommand::Unstake {
            wallet_name,
            amount,
        } => {
            info!("Unstake");
            let submitted = cli.unstake(wallet_name, amount).await?;
            display::submitted_tx("Unstake", &submitted);
            Ok(())
        }
        SubCommand::ViewActiveValidators {} => {
            let validators = cli.view_active_validators().await?;
            display::active_validators(&cli, &node_url, validators).await
        }
        SubCommand::ViewEpoch {} => {
            let (epoch_info, validators) = cli.view_epoch().await?;
            display::epoch(&epoch_info, &validators);
            Ok(())
        }
        SubCommand::GetNamespace { namespace_slug } => {
            let lookup = cli.get_namespace(namespace_slug).await?;
            display::namespace_lookup(&lookup);
            Ok(())
        }
        SubCommand::AddNamespace {
            wallet_name,
            namespace_slug,
            registration_fee,
        } => {
            let resp = cli
                .add_namespace(wallet_name, namespace_slug, registration_fee)
                .await?;
            display::print_registered(&resp);
            Ok(())
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
            let resp = cli
                .post_pinboard_message(eld_client::api::rest::PinboardMessageParams {
                    wallet_name,
                    file_path,
                    content_type,
                    expires_height,
                    visibility,
                    topic,
                    tags,
                    user_fee_amount,
                    namespace,
                })
                .await?;
            display::pinboard_submit(&resp);
            Ok(())
        }
        SubCommand::GetCado { path } => {
            let value = cli.get_cado(path.clone()).await?;
            display::cado(&path, &value);
            Ok(())
        }
        SubCommand::ListCados { search_string } => {
            let paths = cli.list_cados(search_string.clone()).await?;
            display::list_cados(&search_string, &paths);
            Ok(())
        }
        SubCommand::PinboardGetPost { wallet, message_id } => {
            let path = pinboard_post_path(&wallet, &message_id);
            let value = cli.pinboard_get_post(wallet, message_id).await?;
            display::pinboard_post(&path, &value);
            Ok(())
        }
        SubCommand::PinboardListByTag {
            tag,
            page,
            page_size,
        } => {
            let path = pinboard_tag_path(&tag, page, page_size);
            let value = cli.pinboard_list_by_tag(tag, page, page_size).await?;
            display::pinboard_list(&path, &value);
            Ok(())
        }
        SubCommand::PinboardListByWallet {
            wallet,
            page,
            page_size,
        } => {
            let path = pinboard_wallet_path(&wallet, page, page_size);
            let value = cli.pinboard_list_by_wallet(wallet, page, page_size).await?;
            display::pinboard_list(&path, &value);
            Ok(())
        }
    };

    result
}

fn warn_staking_missing(address: &str) {
    tracing::warn!(address = %address, "Staking account not found");
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

fn print_name() {
    info!("\t=                          ");
    info!("\t=== === = = ==   === =   =");
    info!("\t= = === === = =  =   =   =");
    info!("\t=== = = = = = =  === === =");
    info!("\nEld Cli Starting...\n");
}
