use eld_common::error::EldError;
use tracing_subscriber::{fmt::time::UtcTime, prelude::*, EnvFilter};

pub(crate) fn init_default_logging() -> Result<(), EldError> {
    let env_filter = if std::env::var_os("RUST_LOG").is_some() {
        EnvFilter::try_from_default_env().map_err(|e| EldError::InitializationError {
            component: "logging system".to_string(),
            details: format!("Invalid RUST_LOG: {e}"),
        })?
    } else {
        EnvFilter::new("info")
    };

    let subscriber = tracing_subscriber::registry().with(env_filter).with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_timer(UtcTime::rfc_3339())
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_file(false)
            .with_line_number(false),
    );

    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        EldError::InitializationError {
            component: "logging system".to_string(),
            details: format!("Failed to set global default subscriber: {e}"),
        }
    })?;

    Ok(())
}
