use clap::{Parser, Subcommand};
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
    /// Create a local signing wallet.
    CreateWallet {
        /// Name of the wallet.
        #[arg()]
        name: String,
    },
    /// List wallets stored locally
    ListWallets,
    /// Show one local wallet.
    GetWallet {
        /// Wallet name.
        name: String,
    },
    /// Delete a local wallet and its private key.
    RemoveWallet {
        /// Name of the wallet to remove.
        #[arg()]
        name: String,
    },
    /// Sign and commit a transfer from a local wallet.
    #[command(
        after_help = "Example:\n  eld-cli transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000"
    )]
    Transfer {
        /// Wallet that signs and is debited.
        #[arg()]
        wallet_name: String,
        /// Recipient address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        recipient: String,
        /// Amount in base units.
        #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
        amount: u128,
    },
    /// Request tokens from the dev faucet.
    RequestFaucet {
        /// Address that receives the faucet tokens.
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        address: String,
    },
    /// Read an account balance and nonce.
    #[command(
        after_help = "Example:\n  eld-cli get-account 0x1234567890abcdef1234567890abcdef12345678"
    )]
    GetAccount {
        /// Account address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        address: String,
    },
    /// Read a staking account.
    GetStakeAccount {
        /// Staking account address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        address: String,
    },
    /// Read Tendermint ABCI info.
    GetAbciInfo,
    /// Stake tokens from a local wallet.
    Stake {
        /// Wallet that signs and is debited.
        #[arg()]
        wallet_name: String,
        /// Amount in base units.
        #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
        amount: u128,
    },

    /// Unstake tokens back to a local wallet.
    Unstake {
        /// Wallet that signs and receives the unstaked tokens.
        #[arg()]
        wallet_name: String,
        /// Amount in base units.
        #[arg(value_name = "AMOUNT", value_parser = parse_amount)]
        amount: u128,
    },
    /// List validators in the current epoch.
    ViewActiveValidators,
    /// Show epoch metadata and the validator set.
    ViewEpoch,
    /// Look up a namespace slug.
    GetNamespace {
        /// Namespace slug.
        #[arg()]
        namespace_slug: String,
    },
    /// Register a namespace.
    AddNamespace {
        /// Wallet that signs and pays the registration fee.
        #[arg()]
        wallet_name: String,
        /// Namespace slug to register.
        #[arg()]
        namespace_slug: String,
        /// Fee paid to register the namespace (must be greater than zero).
        #[arg(long, default_value_t = 1, value_parser = parse_amount)]
        registration_fee: u128,
    },
    /// Post a pinboard message.
    #[command(
        after_help = "Example:\n  eld-cli post-pinboard-message my-wallet ./message.txt --content-type text/plain"
    )]
    PostPinboardMessage {
        /// Wallet that signs the post.
        #[arg()]
        wallet_name: String,
        /// Path to the message body file.
        #[arg()]
        file_path: String,
        /// MIME type for the message body (`text/plain`, `application/json`, or `image/png`).
        #[arg(long = "content-type", value_parser = parse_content_type)]
        content_type: String,
        /// Post lifetime in blocks. Queries hide the body after the commit height plus this value.
        #[arg(long, default_value_t = 1000)]
        expires_height: u64,
        /// Who can read the post.
        #[arg(long, default_value = "Public", value_parser = parse_visibility)]
        visibility: String,
        /// Optional topic label.
        #[arg(long)]
        topic: Option<String>,
        /// Filter tags (max 4, each max 64 UTF-8 bytes); repeat flag. Address-shaped tags are normalized to 0x + lowercase hex.
        #[arg(long = "tag", action = clap::ArgAction::Append)]
        tags: Vec<String>,
        /// Fee in base units paid with the post.
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
    /// Read a CADO by path.
    GetCado {
        /// CADO path, for example `/@eld/account/0x1234…`.
        #[arg()]
        path: String,
    },
    /// List CADO paths that match a prefix.
    ListCados {
        /// Path prefix to match, for example `/@eld/` or `/@eld/account/`.
        #[arg()]
        search_string: String,
    },
    /// Fetch one pinboard post.
    PinboardGetPost {
        /// Wallet address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        wallet: String,
        /// Message id of the post.
        #[arg()]
        message_id: String,
    },
    /// List pinboard posts by tag.
    PinboardListByTag {
        /// Tag to match.
        #[arg()]
        tag: String,
        /// Zero-based page index.
        #[arg(long, default_value_t = 0)]
        page: usize,
        /// Number of posts per page.
        #[arg(long, default_value_t = 100)]
        page_size: usize,
    },
    /// List pinboard posts by wallet.
    PinboardListByWallet {
        /// Wallet address (`0x` and 40 hex digits).
        #[arg(value_name = "ADDRESS", value_parser = parse_address)]
        wallet: String,
        /// Zero-based page index.
        #[arg(long, default_value_t = 0)]
        page: usize,
        /// Number of posts per page.
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

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: &str = "0x1234567890abcdef1234567890abcdef12345678";

    #[test]
    fn parses_transfer_address_and_amount() {
        let args = Arguments::try_parse_from([
            "eld-cli",
            "transfer",
            "my-wallet",
            "0X1234567890ABCDEF1234567890ABCDEF12345678",
            "1000",
        ])
        .unwrap();

        let SubCommand::Transfer {
            wallet_name,
            recipient,
            amount,
        } = args.cmd
        else {
            panic!("expected transfer");
        };
        assert_eq!(wallet_name, "my-wallet");
        assert_eq!(recipient, ADDRESS);
        assert_eq!(amount, 1000);
    }

    #[test]
    fn rejects_bad_address() {
        let err =
            Arguments::try_parse_from(["eld-cli", "get-account", "not-an-address"]).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("not-an-address"), "{message}");
        assert!(message.contains("address"), "{message}");
    }

    #[test]
    fn rejects_bad_content_type() {
        let err = Arguments::try_parse_from([
            "eld-cli",
            "post-pinboard-message",
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
            "post-pinboard-message",
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
