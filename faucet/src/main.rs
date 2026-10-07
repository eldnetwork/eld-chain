use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer};
use eld_client::config::FeeConfig;
use eld_client::ChainClient;
use eld_common::Address;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

mod config;
mod logging;
mod rate_limiter;

use crate::config::{FaucetConfig, HEALTH_PATH, REQUEST_PATH};

use crate::rate_limiter::{InMemoryRateLimiter, DAILY_REQUEST_LIMIT};

#[derive(Deserialize)]
struct FaucetRequest {
    address: String,
}

#[derive(Serialize)]
struct FaucetResponse {
    success: bool,
    message: Option<String>,
}

#[derive(Clone)]
struct AppState {
    client: ChainClient,
    rate_limiter: Arc<InMemoryRateLimiter>,
    wallet_name: String,
    drip_base_units: u64,
}

fn failure(status: actix_web::http::StatusCode, message: impl Into<String>) -> HttpResponse {
    HttpResponse::build(status).json(FaucetResponse {
        success: false,
        message: Some(message.into()),
    })
}

async fn request_tokens(
    body: web::Json<FaucetRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if Address::parse_hex_str(&body.address).is_err() {
        return failure(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid address format",
        );
    }

    if let Err(remaining_allowed) = state
        .rate_limiter
        .check_and_consume(&body.address, state.drip_base_units)
        .await
    {
        return failure(
            actix_web::http::StatusCode::TOO_MANY_REQUESTS,
            format!(
                "Daily faucet limit reached for this address. Remaining today: {remaining_allowed} base units."
            ),
        );
    }

    match state
        .client
        .transfer(
            state.wallet_name.clone(),
            body.address.clone(),
            u128::from(state.drip_base_units),
        )
        .await
    {
        Ok(_) => {
            info!("Faucet transfer tx submitted");
            HttpResponse::Ok().json(FaucetResponse {
                success: true,
                message: Some("tx submit".to_string()),
            })
        }
        Err(e) => {
            error!("Faucet transfer failed: {e}");
            failure(
                actix_web::http::StatusCode::BAD_REQUEST,
                "Error submitting tx",
            )
        }
    }
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().body("OK")
}

fn io_err(err: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    logging::init_default_logging().map_err(io_err)?;

    info!("Starting faucet server");
    let faucet_config = FaucetConfig::load().map_err(io_err)?;
    let tendermint_url = faucet_config.node_url().map_err(io_err)?;
    let bind_addr = faucet_config.bind_addr();
    info!("Connecting to Tendermint at {tendermint_url}");
    info!("Starting faucet service at {bind_addr}");
    info!(
        chain_id = %faucet_config.chain_id,
        drip_base_units = faucet_config.drip_base_units,
        address_daily_drips = faucet_config.address_daily_drips,
        ip_hourly_requests = faucet_config.ip_hourly_requests,
        hot_wallet_reserve = faucet_config.hot_wallet_reserve,
        db_path = %faucet_config.db_path,
        wallet_path = %faucet_config.wallet_path.display(),
        wallet_name = %faucet_config.wallet_name,
        "Faucet config loaded"
    );

    let client = ChainClient::with_wallets(
        faucet_config.to_client_config(),
        FeeConfig::default(),
        &faucet_config.wallet_path,
    )
    .map_err(io_err)?;

    match client
        .get_wallet_by_name(faucet_config.wallet_name.clone())
        .await
    {
        Ok(Some(_)) => {
            info!(
                "Faucet wallet '{}' loaded from {}",
                faucet_config.wallet_name,
                faucet_config.wallet_path.display()
            );
        }
        Ok(None) => {
            return Err(io_err(format!(
                "Faucet wallet '{}' not found in {}",
                faucet_config.wallet_name,
                faucet_config.wallet_path.display()
            )));
        }
        Err(e) => {
            return Err(io_err(format!(
                "Failed to load faucet wallet '{}' from {}: {e}",
                faucet_config.wallet_name,
                faucet_config.wallet_path.display()
            )));
        }
    }

    let state = AppState {
        client,
        rate_limiter: Arc::new(InMemoryRateLimiter::new(DAILY_REQUEST_LIMIT)),
        wallet_name: faucet_config.wallet_name.clone(),
        drip_base_units: faucet_config.drip_base_units,
    };

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(state.clone()))
            .route(HEALTH_PATH, web::get().to(health_check))
            .route(REQUEST_PATH, web::post().to(request_tokens))
    })
    .bind(bind_addr)?
    .run()
    .await
}
