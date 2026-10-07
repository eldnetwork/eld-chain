use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer};
use eld_client::config::{get_client_setup, WALLETS_PATH};
use eld_client::ChainClient;
use eld_common::Address;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

mod logging;
mod rate_limiter;

use crate::rate_limiter::{InMemoryRateLimiter, DAILY_REQUEST_LIMIT};

const FAUCET_WALLET_NAME: &str = "wallet-faucet-1";
const FAUCET_DRIP_BASE_UNITS: u64 = 1_000 * 1_000_000;

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
    match state
        .client
        .get_wallet_by_name(FAUCET_WALLET_NAME.to_string())
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            error!("Faucet wallet not found");
            return failure(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Faucet wallet not found",
            );
        }
        Err(e) => {
            error!("Failed to load faucet wallet: {e}");
            return failure(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load faucet wallet",
            );
        }
    }

    if Address::parse_hex_str(&body.address).is_err() {
        return failure(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid address format",
        );
    }

    if let Err(remaining_allowed) = state
        .rate_limiter
        .check_and_consume(&body.address, FAUCET_DRIP_BASE_UNITS)
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
            FAUCET_WALLET_NAME.to_string(),
            body.address.clone(),
            u128::from(FAUCET_DRIP_BASE_UNITS),
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
    let setup = get_client_setup().map_err(io_err)?;
    let tendermint_url = setup.config.get_node_url().map_err(io_err)?;
    let bind_addr = format!("{}:{}", setup.config.faucet_host, setup.config.faucet_port);
    let faucet_end_point = setup.config.faucet_end_point.clone();
    info!("Connecting to Tendermint at {tendermint_url}");
    info!("Starting faucet service at {bind_addr}");

    let client =
        ChainClient::with_wallets(setup.config, setup.fee_config, WALLETS_PATH).map_err(io_err)?;
    let state = AppState {
        client,
        rate_limiter: Arc::new(InMemoryRateLimiter::new(DAILY_REQUEST_LIMIT)),
    };

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(state.clone()))
            .route("/health", web::get().to(health_check))
            .route(faucet_end_point.as_str(), web::post().to(request_tokens))
    })
    .bind(bind_addr)?
    .run()
    .await
}
