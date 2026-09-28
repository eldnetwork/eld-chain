use clap::{Parser, Subcommand};
use eld_common::coin::Coin;
use eld_common::error::EldError;
use eld_common::tx::validate_post_message_content_type;
use eld_common::Address;

#[derive(Parser, Debug)]
#[command(author = "ELD LABS", version, about = "Eld Cli")]
pub struct Arguments {
    #[command(subcommand)]
    pub(crate) cmd: SubCommand,

    /// Config file to use (default: config/config.json)
    #[arg(long = "cli-config", default_value = "config/config.json")]
    pub(crate) config: String,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SubCommand {
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
    #[command(
        after_help = "Example:\n  eld-cli transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000"
    )]
    Transfer {
        #[arg()]
        // wallet which will sign and be debited
        wallet_name: String,
        /// Recipient address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        recipient: String,
        /// Amount in base units.
        #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
        amount: u128,
    },
    /// Request tokens from the faucet
    /// Example: cargo run request-faucet 0x1234567890abcdef1234567890abcdef12345678
    RequestFaucet {
        /// Address that receives the faucet tokens.
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        address: String,
    },
    /// Get Account (Account is a read only view of a wallet, fetched from a node)
    #[command(
        after_help = "Example:\n  eld-cli get-account 0x1234567890abcdef1234567890abcdef12345678"
    )]
    GetAccount {
        /// Account address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        address: String,
    },
    GetStakeAccount {
        /// Staking account address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
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
        /// Amount in base units.
        #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
        amount: u128,
    },

    /// Unstake tokens
    Unstake {
        #[arg()]
        // wallet which will sign and receive funds
        wallet_name: String,
        /// Amount in base units.
        #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
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
        #[arg(long, default_value_t = 1, value_parser = parse_amount)]
        registration_fee: u128,
    },
    /// Post a pinboard message (user-signed blob + node broadcasts PostMessage tx)
    #[command(
        after_help = "Example:\n  eld-cli post-pinboard-message my-wallet ./message.txt --content-type text/plain"
    )]
    PostPinboardMessage {
        #[arg()]
        wallet_name: String,
        #[arg()]
        file_path: String,
        /// MIME type for the message body (`text/plain`, `application/json`, or `image/png`).
        #[arg(long = "content-type", value_parser = parse_content_type)]
        content_type: String,
        /// Post lifetime in blocks (TTL). Pinboard queries hide the body after `committed_height + ttl`.
        /// If `ttl` is `0`, it defaults to `1000` blocks.
        #[arg(long, default_value_t = 1000)]
        expires_height: u64,
        #[arg(long, default_value = "Public", value_parser = parse_visibility)]
        visibility: String,
        #[arg(long)]
        topic: Option<String>,
        /// Filter tags (max 4, each max 64 UTF-8 bytes); repeat flag. Address-shaped tags are normalized to 0x + lowercase hex.
        #[arg(long = "tag", action = clap::ArgAction::Append)]
        tags: Vec<String>,
        #[arg(
            long,
            default_value_t = 1000,
            value_name = "AMOUNT",
            value_parser = parse_amount
        )]
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
        /// Wallet address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
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
        /// Wallet address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        wallet: String,
        #[arg(long, default_value_t = 0)]
        page: usize,
        #[arg(long, default_value_t = 100)]
        page_size: usize,
    },
}

fn parse_address(raw: &str) -> Result<String, EldError> {
    Ok(Address::parse_hex_str(raw)?.hex_with_prefix())
}

fn parse_amount(raw: &str) -> Result<u128, EldError> {
    let amount = raw.parse::<u128>().map_err(|err| EldError::CoinError {
        details: format!("invalid amount '{raw}': {err}"),
    })?;
    Ok(Coin::new(amount)?.amount())
}

fn parse_content_type(raw: &str) -> Result<String, EldError> {
    validate_post_message_content_type(raw)?;
    Ok(raw.trim().to_string())
}

fn parse_visibility(raw: &str) -> Result<String, EldError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(EldError::ValidationError {
            field: "visibility".to_string(),
            value: raw.to_string(),
            details: "visibility must not be empty".to_string(),
        });
    }
    Ok(trimmed.to_string())
}
