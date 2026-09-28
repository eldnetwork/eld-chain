use clap::{Parser, Subcommand};

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
