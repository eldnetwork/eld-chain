use clap::{Args, Parser, Subcommand, ValueEnum};
use eld_common::coin::Coin;
use eld_common::error::EldError;
use eld_common::tx::validate_post_message_content_type;
use eld_common::Address;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "eld-cli", author, version, about)]
pub struct Arguments {
    #[command(subcommand)]
    pub(crate) cmd: SubCommand,

    /// Wallet file. Linux: `~/.local/share/eld/wallets.json`. macOS: `~/Library/Application Support/eld/wallets.json`.
    #[arg(long, value_name = "PATH")]
    pub(crate) wallets: Option<PathBuf>,

    /// Client endpoints file. Linux: `~/.config/eld/eld-cli-config.json`. macOS: `~/Library/Application Support/eld/eld-cli-config.json`.
    #[arg(long = "cli-config", env = "ELD_CLI_CONFIG", value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    /// Skip confirmations.
    #[arg(long, short = 'y')]
    pub(crate) yes: bool,

    /// Output format. `text` is the default. Also read from `ELD_CLI_OUTPUT`.
    #[arg(long, value_enum, default_value = "text", env = "ELD_CLI_OUTPUT")]
    pub(crate) output: OutputFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SubCommand {
    /// Local signing keys.
    Wallet {
        #[command(subcommand)]
        cmd: WalletCommand,
    },
    /// Balances and nonces.
    Account {
        #[command(subcommand)]
        cmd: AccountCommand,
    },
    /// Sign and broadcast.
    Tx {
        #[command(subcommand)]
        cmd: TxCommand,
    },
    /// Request tokens from the dev faucet.
    #[command(
        after_help = "Example:\n  eld-cli faucet 0x1234567890abcdef1234567890abcdef12345678\n  eld-cli faucet my-wallet"
    )]
    Faucet(FaucetArgs),
    /// Epoch, validators, and ABCI info.
    Chain {
        #[command(subcommand)]
        cmd: ChainCommand,
    },
    /// Namespace registry.
    Namespace {
        #[command(subcommand)]
        cmd: NamespaceCommand,
    },
    /// Ephemeral posts.
    Pinboard {
        #[command(subcommand)]
        cmd: PinboardCommand,
    },
    /// Content-addressed objects.
    Cado {
        #[command(subcommand)]
        cmd: CadoCommand,
    },
    /// Node and faucet addresses.
    Config {
        #[command(subcommand)]
        cmd: ConfigCommand,
    },
    /// Delete the client config and wallet file.
    Reset,

    // Flat names from the previous CLI. Hidden so `--help` lists groups only.
    /// Create a local signing wallet.
    #[command(hide = true)]
    CreateWallet(CreateWalletArgs),
    /// List wallets stored locally
    #[command(hide = true)]
    ListWallets,
    /// Show one local wallet.
    #[command(hide = true)]
    GetWallet(ShowWalletArgs),
    /// Delete a local wallet and its private key.
    #[command(hide = true)]
    RemoveWallet(RemoveWalletArgs),
    /// Sign and commit a transfer from a local wallet.
    #[command(
        hide = true,
        after_help = "Example:\n  eld-cli transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000"
    )]
    Transfer(TransferArgs),
    /// Request tokens from the dev faucet.
    #[command(hide = true)]
    RequestFaucet(FaucetArgs),
    /// Read an account balance and nonce.
    #[command(
        hide = true,
        after_help = "Example:\n  eld-cli get-account 0x1234567890abcdef1234567890abcdef12345678"
    )]
    GetAccount(GetAccountArgs),
    /// Read a staking account.
    #[command(hide = true)]
    GetStakeAccount(GetStakeAccountArgs),
    /// Read Tendermint ABCI info.
    #[command(hide = true)]
    GetAbciInfo,
    /// Stake tokens from a local wallet.
    #[command(hide = true)]
    Stake(StakeArgs),
    /// Unstake tokens back to a local wallet.
    #[command(hide = true)]
    Unstake(UnstakeArgs),
    /// List validators in the current epoch.
    #[command(hide = true)]
    ViewActiveValidators,
    /// Show epoch metadata and the validator set.
    #[command(hide = true)]
    ViewEpoch,
    /// Look up a namespace slug.
    #[command(hide = true)]
    GetNamespace(NamespaceSlugArgs),
    /// Register a namespace.
    #[command(hide = true)]
    AddNamespace(AddNamespaceArgs),
    /// Post a pinboard message.
    #[command(
        hide = true,
        after_help = "Example:\n  eld-cli post-pinboard-message my-wallet ./message.txt --content-type text/plain"
    )]
    PostPinboardMessage(PostPinboardArgs),
    /// Read a CADO by path.
    #[command(hide = true)]
    GetCado(GetCadoArgs),
    /// List CADO paths that match a prefix.
    #[command(hide = true)]
    ListCados(ListCadosArgs),
    /// Fetch one pinboard post.
    #[command(hide = true)]
    PinboardGetPost(PinboardGetPostArgs),
    /// List pinboard posts by tag.
    #[command(hide = true)]
    PinboardListByTag(ListByTagArgs),
    /// List pinboard posts by wallet.
    #[command(hide = true)]
    PinboardListByWallet(ListByWalletArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum WalletCommand {
    /// Create a local signing wallet.
    Create(CreateWalletArgs),
    /// List wallets stored locally
    List,
    /// Show one local wallet.
    Show(ShowWalletArgs),
    /// Delete a local wallet and its private key.
    Remove(RemoveWalletArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum AccountCommand {
    /// Read an account balance and nonce.
    #[command(
        after_help = "Example:\n  eld-cli account get 0x1234567890abcdef1234567890abcdef12345678"
    )]
    Get(GetAccountArgs),
    /// Read a staking account.
    StakeGet(GetStakeAccountArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum TxCommand {
    /// Sign and commit a transfer from a local wallet.
    #[command(
        after_help = "Example:\n  eld-cli tx transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000"
    )]
    Transfer(TransferArgs),
    /// Stake tokens from a local wallet.
    Stake(StakeArgs),
    /// Unstake tokens back to a local wallet.
    Unstake(UnstakeArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigCommand {
    /// Set the node IP or URL and read `chain_id` from the node.
    #[command(
        after_help = "Example:\n  eld-cli config node 127.0.0.1\n  eld-cli config node https://rpc.example.com"
    )]
    Node {
        /// IP, hostname, or http(s) URL.
        address: String,
    },
    /// Set the faucet IP or URL.
    #[command(
        after_help = "Example:\n  eld-cli config faucet 127.0.0.1\n  eld-cli config faucet https://faucet.example.com"
    )]
    Faucet {
        /// IP, hostname, or http(s) URL.
        address: String,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum ChainCommand {
    /// Read Tendermint ABCI info.
    AbciInfo,
    /// Show epoch metadata and the validator set.
    Epoch,
    /// List validators in the current epoch.
    Validators,
}

#[derive(Subcommand, Debug)]
pub(crate) enum NamespaceCommand {
    /// Look up a namespace slug.
    Get(NamespaceSlugArgs),
    /// Register a namespace.
    Add(AddNamespaceArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum PinboardCommand {
    /// Post a pinboard message.
    #[command(
        after_help = "Example:\n  eld-cli pinboard post my-wallet ./message.txt --content-type text/plain"
    )]
    Post(PostPinboardArgs),
    /// Fetch one pinboard post.
    Get(PinboardGetPostArgs),
    /// List pinboard posts by tag.
    ListTag(ListByTagArgs),
    /// List pinboard posts by wallet address.
    ListByAddress(ListByWalletArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum CadoCommand {
    /// Read a CADO by path.
    Get(GetCadoArgs),
    /// List CADO paths that match a prefix.
    List(ListCadosArgs),
}

#[derive(Args, Debug)]
pub(crate) struct CreateWalletArgs {
    /// Name of the wallet.
    pub(crate) name: String,
}

#[derive(Args, Debug)]
pub(crate) struct ShowWalletArgs {
    /// Wallet name.
    pub(crate) name: String,
}

#[derive(Args, Debug)]
pub(crate) struct RemoveWalletArgs {
    /// Name of the wallet to remove.
    pub(crate) name: String,
}

#[derive(Args, Debug)]
pub(crate) struct TransferArgs {
    /// Wallet that signs and is debited.
    pub(crate) wallet_name: String,
    /// Recipient address (`0x` and 40 hex digits).
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) recipient: Address,
    /// Amount in base units.
    #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
    pub(crate) amount: Coin,
}

#[derive(Args, Debug)]
pub(crate) struct FaucetArgs {
    /// Hex address (`0x` and 40 hex digits) or local wallet name.
    #[arg(value_name = "ADDRESS_OR_NAME")]
    pub(crate) recipient: String,
}

#[derive(Args, Debug)]
pub(crate) struct GetAccountArgs {
    /// Account address (`0x` and 40 hex digits).
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) address: Address,
}

#[derive(Args, Debug)]
pub(crate) struct GetStakeAccountArgs {
    /// Staking account address (`0x` and 40 hex digits).
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) address: Address,
}

#[derive(Args, Debug)]
pub(crate) struct StakeArgs {
    /// Wallet that signs and is debited.
    pub(crate) wallet_name: String,
    /// Amount in base units.
    #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
    pub(crate) amount: Coin,
}

#[derive(Args, Debug)]
pub(crate) struct UnstakeArgs {
    /// Wallet that signs and receives the unstaked tokens.
    pub(crate) wallet_name: String,
    /// Amount in base units.
    #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
    pub(crate) amount: Coin,
}

#[derive(Args, Debug)]
pub(crate) struct NamespaceSlugArgs {
    /// Namespace slug.
    pub(crate) namespace_slug: String,
}

#[derive(Args, Debug)]
pub(crate) struct AddNamespaceArgs {
    /// Wallet that signs and pays the registration fee.
    pub(crate) wallet_name: String,
    /// Namespace slug to register.
    pub(crate) namespace_slug: String,
    /// Fee paid to register the namespace (must be greater than zero).
    #[arg(long, default_value = "1", value_parser = parse_amount)]
    pub(crate) registration_fee: Coin,
}

#[derive(Args, Debug)]
pub(crate) struct PostPinboardArgs {
    /// Wallet that signs the post.
    pub(crate) wallet_name: String,
    /// Path to the message body file.
    pub(crate) file_path: String,
    /// MIME type for the message body (`text/plain`, `application/json`, or `image/png`).
    #[arg(long = "content-type", value_parser = parse_content_type)]
    pub(crate) content_type: String,
    /// Post lifetime in blocks. Queries hide the body after the commit height plus this value.
    #[arg(long, default_value_t = 1000)]
    pub(crate) expires_height: u64,
    /// Who can read the post (`Public` or `public`).
    #[arg(long, default_value = "Public", value_parser = parse_visibility)]
    pub(crate) visibility: String,
    /// Optional topic label.
    #[arg(long)]
    pub(crate) topic: Option<String>,
    /// Filter tags (max 4, each max 64 UTF-8 bytes); repeat flag. Address-shaped tags are normalized to 0x + lowercase hex.
    #[arg(long = "tag", action = clap::ArgAction::Append)]
    pub(crate) tags: Vec<String>,
    /// Fee in base units paid with the post.
    #[arg(
        long,
        default_value = "1000",
        value_name = "AMOUNT",
        value_parser = parse_amount
    )]
    pub(crate) user_fee_amount: Coin,
    /// Custom namespace slug (letter-only; must be registered and owned by the posting wallet).
    #[arg(long)]
    pub(crate) namespace: Option<String>,
}

#[derive(Args, Debug)]
pub(crate) struct GetCadoArgs {
    /// CADO path, for example `/@eld/account/0x1234…`.
    pub(crate) path: String,
}

#[derive(Args, Debug)]
pub(crate) struct ListCadosArgs {
    /// Path prefix to match, for example `/@eld/` or `/@eld/account/`.
    pub(crate) search_string: String,
}

#[derive(Args, Debug)]
pub(crate) struct PinboardGetPostArgs {
    /// Wallet address (`0x` and 40 hex digits).
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) wallet: Address,
    /// Message id of the post.
    pub(crate) message_id: String,
}

#[derive(Args, Debug)]
pub(crate) struct ListByTagArgs {
    /// Tag to match.
    pub(crate) tag: String,
    /// Zero-based page index.
    #[arg(long, default_value_t = 0)]
    pub(crate) page: usize,
    /// Number of posts per page.
    #[arg(long, default_value_t = 100)]
    pub(crate) page_size: usize,
}

#[derive(Args, Debug)]
pub(crate) struct ListByWalletArgs {
    /// Wallet address (`0x` and 40 hex digits).
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) wallet: Address,
    /// Zero-based page index.
    #[arg(long, default_value_t = 0)]
    pub(crate) page: usize,
    /// Number of posts per page.
    #[arg(long, default_value_t = 100)]
    pub(crate) page_size: usize,
}

fn parse_address(raw: &str) -> Result<Address, EldError> {
    Address::parse_hex_str(raw)
}

fn parse_amount(raw: &str) -> Result<Coin, EldError> {
    let amount = raw.parse::<u128>().map_err(|err| EldError::CoinError {
        details: format!("invalid amount '{raw}': {err}"),
    })?;
    Coin::new(amount)
}

fn parse_content_type(raw: &str) -> Result<String, EldError> {
    validate_post_message_content_type(raw)?;
    Ok(raw.trim().to_string())
}

/// Spellings used for pinboard visibility in this repo. `eld-common` stores the field as a
/// free-form string and has no visibility enum.
const PINBOARD_VISIBILITY: &[&str] = &["Public", "public"];

fn parse_visibility(raw: &str) -> Result<String, EldError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(EldError::ValidationError {
            field: "visibility".to_string(),
            value: raw.to_string(),
            details: "visibility must not be empty".to_string(),
        });
    }
    if !PINBOARD_VISIBILITY.contains(&trimmed) {
        return Err(EldError::ValidationError {
            field: "visibility".to_string(),
            value: raw.to_string(),
            details: "visibility must be Public or public".to_string(),
        });
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    const ADDRESS: &str = "0x1234567890abcdef1234567890abcdef12345678";

    #[test]
    fn parses_transfer_address_and_amount() {
        let args = Arguments::try_parse_from([
            "eld-cli",
            "tx",
            "transfer",
            "my-wallet",
            "0X1234567890ABCDEF1234567890ABCDEF12345678",
            "1000",
        ])
        .unwrap();

        let SubCommand::Tx {
            cmd: TxCommand::Transfer(transfer),
        } = args.cmd
        else {
            panic!("expected transfer");
        };
        assert_eq!(transfer.wallet_name, "my-wallet");
        assert_eq!(transfer.recipient.hex_with_prefix(), ADDRESS);
        assert_eq!(transfer.amount.amount(), 1000);
    }

    #[test]
    fn parses_legacy_transfer_alias() {
        let args = Arguments::try_parse_from([
            "eld-cli",
            "transfer",
            "my-wallet",
            "0X1234567890ABCDEF1234567890ABCDEF12345678",
            "1000",
        ])
        .unwrap();

        let SubCommand::Transfer(transfer) = args.cmd else {
            panic!("expected legacy transfer alias");
        };
        assert_eq!(transfer.wallet_name, "my-wallet");
        assert_eq!(transfer.recipient.hex_with_prefix(), ADDRESS);
        assert_eq!(transfer.amount.amount(), 1000);
    }

    #[test]
    fn parses_faucet_hex_and_wallet_name() {
        let hex = Arguments::try_parse_from(["eld-cli", "faucet", ADDRESS]).unwrap();
        let SubCommand::Faucet(faucet) = hex.cmd else {
            panic!("expected faucet");
        };
        assert_eq!(faucet.recipient, ADDRESS);

        let named = Arguments::try_parse_from(["eld-cli", "faucet", "my-wallet"]).unwrap();
        let SubCommand::Faucet(faucet) = named.cmd else {
            panic!("expected faucet");
        };
        assert_eq!(faucet.recipient, "my-wallet");
    }

    #[test]
    fn faucet_is_not_a_tx_subcommand() {
        let err = Arguments::try_parse_from(["eld-cli", "tx", "faucet", ADDRESS]).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("unrecognized subcommand"), "{message}");
    }

    #[test]
    fn rejects_bad_address() {
        let err =
            Arguments::try_parse_from(["eld-cli", "account", "get", "not-an-address"]).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("not-an-address"), "{message}");
        assert!(message.contains("address"), "{message}");
    }

    #[test]
    fn rejects_bad_content_type() {
        let err = Arguments::try_parse_from([
            "eld-cli",
            "pinboard",
            "post",
            "my-wallet",
            "./message.txt",
            "--content-type",
            "text/html",
        ])
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("text/html"), "{message}");
        assert!(message.contains("unsupported mime type"), "{message}");
    }

    #[test]
    fn rejects_empty_visibility() {
        let err = Arguments::try_parse_from([
            "eld-cli",
            "pinboard",
            "post",
            "my-wallet",
            "./message.txt",
            "--content-type",
            "text/plain",
            "--visibility",
            " ",
        ])
        .unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("visibility must not be empty"),
            "{message}"
        );
    }

    const HELP_ROOT: &str = "\
Command-line client for an Eld node: local wallets, signed transactions, and chain queries

Usage: eld-cli [OPTIONS] <COMMAND>

Commands:
  wallet     Local signing keys
  account    Balances and nonces
  tx         Sign and broadcast
  faucet     Request tokens from the dev faucet
  chain      Epoch, validators, and ABCI info
  namespace  Namespace registry
  pinboard   Ephemeral posts
  cado       Content-addressed objects
  config     Node and faucet addresses
  reset      Delete the client config and wallet file
  help       Print this message or the help of the given subcommand(s)

Options:
      --wallets <PATH>     Wallet file. Linux: `~/.local/share/eld/wallets.json`. macOS:
                           `~/Library/Application Support/eld/wallets.json`
      --cli-config <PATH>  Client endpoints file. Linux: `~/.config/eld/eld-cli-config.json`. macOS:
                           `~/Library/Application Support/eld/eld-cli-config.json` [env:
                           ELD_CLI_CONFIG=]
  -y, --yes                Skip confirmations
      --output <OUTPUT>    Output format. `text` is the default. Also read from `ELD_CLI_OUTPUT`
                           [env: ELD_CLI_OUTPUT=] [default: text] [possible values: text, json]
  -h, --help               Print help
  -V, --version            Print version
";

    const HELP_WALLET: &str = "\
Local signing keys

Usage: wallet <COMMAND>

Commands:
  create  Create a local signing wallet
  list    List wallets stored locally
  show    Show one local wallet
  remove  Delete a local wallet and its private key
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
";

    #[test]
    fn help_text_matches_root_and_wallet() {
        assert_eq!(rendered_help(None), HELP_ROOT);
        assert_eq!(rendered_help(Some("wallet")), HELP_WALLET);
    }

    fn rendered_help(subcommand: Option<&str>) -> String {
        let mut cmd = Arguments::command()
            .term_width(100)
            .color(clap::ColorChoice::Never)
            .styles(clap::builder::Styles::plain());
        if let Some(name) = subcommand {
            cmd = cmd
                .find_subcommand(name)
                .unwrap_or_else(|| panic!("missing subcommand {name}"))
                .clone()
                .term_width(100)
                .color(clap::ColorChoice::Never)
                .styles(clap::builder::Styles::plain());
        }
        let mut text = cmd.render_help().to_string();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text
    }

    #[test]
    fn rejects_invalid_visibility() {
        let err = Arguments::try_parse_from([
            "eld-cli",
            "pinboard",
            "post",
            "my-wallet",
            "./message.txt",
            "--content-type",
            "text/plain",
            "--visibility",
            "secret",
        ])
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("secret"), "{message}");
        assert!(message.contains("Public or public"), "{message}");
    }
}
