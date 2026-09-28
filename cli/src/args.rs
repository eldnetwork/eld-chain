use clap::{Args, Parser, Subcommand};
use eld_common::coin::Coin;
use eld_common::error::EldError;
use eld_common::tx::validate_post_message_content_type;
use eld_common::Address;

#[derive(Parser, Debug)]
#[command(name = "eld-cli", author, version, about)]
pub struct Arguments {
    #[command(subcommand)]
    pub(crate) cmd: SubCommand,

    /// Config file to use (default: config/config.json)
    #[arg(
        long = "cli-config",
        env = "ELD_CLI_CONFIG",
        default_value = "config/config.json"
    )]
    pub(crate) config: String,
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
    /// Request tokens from the dev faucet.
    #[command(alias = "request-faucet")]
    Faucet(FaucetArgs),
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
    /// List pinboard posts by wallet.
    ListWallet(ListByWalletArgs),
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
    pub(crate) recipient: String,
    /// Amount in base units.
    #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
    pub(crate) amount: u128,
}

#[derive(Args, Debug)]
pub(crate) struct FaucetArgs {
    /// Address that receives the faucet tokens.
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) address: String,
}

#[derive(Args, Debug)]
pub(crate) struct GetAccountArgs {
    /// Account address (`0x` and 40 hex digits).
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) address: String,
}

#[derive(Args, Debug)]
pub(crate) struct GetStakeAccountArgs {
    /// Staking account address (`0x` and 40 hex digits).
    #[arg(value_name = "ADDRESS", value_parser = parse_address)]
    pub(crate) address: String,
}

#[derive(Args, Debug)]
pub(crate) struct StakeArgs {
    /// Wallet that signs and is debited.
    pub(crate) wallet_name: String,
    /// Amount in base units.
    #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
    pub(crate) amount: u128,
}

#[derive(Args, Debug)]
pub(crate) struct UnstakeArgs {
    /// Wallet that signs and receives the unstaked tokens.
    pub(crate) wallet_name: String,
    /// Amount in base units.
    #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
    pub(crate) amount: u128,
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
    #[arg(long, default_value_t = 1, value_parser = parse_amount)]
    pub(crate) registration_fee: u128,
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
    /// Who can read the post.
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
        default_value_t = 1000,
        value_name = "AMOUNT",
        value_parser = parse_amount
    )]
    pub(crate) user_fee_amount: u128,
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
    pub(crate) wallet: String,
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
    pub(crate) wallet: String,
    /// Zero-based page index.
    #[arg(long, default_value_t = 0)]
    pub(crate) page: usize,
    /// Number of posts per page.
    #[arg(long, default_value_t = 100)]
    pub(crate) page_size: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(transfer.recipient, ADDRESS);
        assert_eq!(transfer.amount, 1000);
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
        assert_eq!(transfer.recipient, ADDRESS);
        assert_eq!(transfer.amount, 1000);
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
}
