use eld_common::error::EldError;
use tracing::info;
use tracing_subscriber::{fmt::time::UtcTime, prelude::*, EnvFilter};

pub(crate) fn init_default_logging() -> Result<(), EldError> {
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

pub(crate) fn print_name() {
    info!("\t=                          ");
    info!("\t=== === = = ==   === =   =");
    info!("\t= = === === = =  =   =   =");
    info!("\t=== = = = = = =  === === =");
    info!("\nEld Cli Starting...\n");
}
