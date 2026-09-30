use eld_common::error::{EldError, ErrorBuilder};
use std::path::Path;

use crate::output::OutputMode;

pub(crate) fn reset(
    config_path: &Path,
    wallets_path: &Path,
    yes: bool,
    interactive: bool,
    mode: OutputMode,
) -> Result<(), EldError> {
    warn(config_path, wallets_path);
    confirm(yes, interactive)?;
    let config_deleted = remove_if_present(config_path)?;
    let wallets_deleted = remove_if_present(wallets_path)?;
    let text = format!(
        "config: {} ({})\nwallets: {} ({})",
        config_path.display(),
        presence(config_deleted),
        wallets_path.display(),
        presence(wallets_deleted),
    );
    crate::output::emit(
        mode,
        &text,
        &ResetJson {
            config: FileJson {
                path: config_path.display().to_string(),
                deleted: config_deleted,
            },
            wallets: FileJson {
                path: wallets_path.display().to_string(),
                deleted: wallets_deleted,
            },
        },
    )
}

fn presence(deleted: bool) -> &'static str {
    if deleted {
        "deleted"
    } else {
        "absent"
    }
}

fn warn(config_path: &Path, wallets_path: &Path) {
    eprintln!(
        "This deletes the client config and the wallet file. The wallet file holds unencrypted private keys."
    );
    eprintln!("config: {}", config_path.display());
    eprintln!("wallets: {}", wallets_path.display());
}

fn confirm(yes: bool, interactive: bool) -> Result<(), EldError> {
    if yes {
        return Ok(());
    }
    if !interactive {
        return Err(ErrorBuilder::validation_error(
            "yes",
            "reset",
            "refusing to delete config and wallets without --yes when not running in a terminal",
        ));
    }
    eprintln!("Delete these files? [y/N]");
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).map_err(|err| {
        ErrorBuilder::validation_error(
            "yes",
            "reset",
            &format!("failed to read confirmation: {err}"),
        )
    })?;
    if !matches!(answer.trim(), "y" | "Y") {
        return Err(ErrorBuilder::validation_error(
            "yes",
            "reset",
            "reset cancelled",
        ));
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<bool, EldError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(ErrorBuilder::file_system_error(
            "remove",
            &path.display().to_string(),
            &err.to_string(),
        )),
    }
}

#[derive(serde::Serialize)]
struct ResetJson {
    config: FileJson,
    wallets: FileJson,
}

#[derive(serde::Serialize)]
struct FileJson {
    path: String,
    deleted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_without_yes_in_non_tty_does_not_delete() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("eld-cli-config.json");
        let wallets = dir.path().join("wallets.json");
        std::fs::write(&config, "{}").unwrap();
        std::fs::write(&wallets, "[]").unwrap();

        let err = reset(&config, &wallets, false, false, OutputMode::text()).unwrap_err();
        assert!(err.to_string().contains("--yes"), "{err}");
        assert!(config.exists());
        assert!(wallets.exists());
    }

    #[test]
    fn reset_with_yes_deletes_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("eld-cli-config.json");
        let wallets = dir.path().join("wallets.json");
        std::fs::write(&config, "{}").unwrap();
        std::fs::write(&wallets, "[]").unwrap();

        reset(&config, &wallets, true, false, OutputMode::text()).unwrap();
        assert!(!config.exists());
        assert!(!wallets.exists());
    }

    #[test]
    fn reset_with_yes_allows_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("eld-cli-config.json");
        let wallets = dir.path().join("wallets.json");
        reset(&config, &wallets, true, false, OutputMode::text()).unwrap();
    }
}
