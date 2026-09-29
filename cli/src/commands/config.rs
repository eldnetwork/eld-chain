use eld_common::error::EldError;
use std::path::Path;

use crate::args::ConfigCommand;
use crate::setup;

pub(crate) async fn run(path: &Path, cmd: ConfigCommand) -> Result<(), EldError> {
    match cmd {
        ConfigCommand::Node { address } => setup::configure_node(path, &address).await,
        ConfigCommand::Faucet { address } => setup::configure_faucet(path, &address).await,
    }
}
