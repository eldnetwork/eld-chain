use actix_cors::Cors;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use eld_client::config::FeeConfig;
use eld_client::ChainClient;
use eld_common::error::EldError;
use eld_common::Address;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info};

mod config;
mod logging;
mod store;

use crate::config::{FaucetConfig, HEALTH_PATH, REQUEST_PATH};

use crate::store::{FaucetStore, LimitKind, StoreError};

const JSON_BODY_LIMIT: usize = 1024;
const TRUST_PROXY_ENV: &str = "FAUCET_TRUST_PROXY";
const BALANCE_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct FaucetRequest {
    address: String,
}

#[derive(Serialize)]
struct FaucetResponse {
    success: bool,
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<String>,
}

struct BalanceCache {
    inner: Mutex<Option<(Instant, u128)>>,
}

impl BalanceCache {
    fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    fn get(&self, ttl: Duration) -> Option<u128> {
        let guard = self.inner.lock().expect("balance cache mutex");
        match *guard {
            Some((fetched_at, balance)) if fetched_at.elapsed() < ttl => Some(balance),
            _ => None,
        }
    }

    fn set(&self, balance: u128) {
        *self.inner.lock().expect("balance cache mutex") = Some((Instant::now(), balance));
    }

    fn subtract(&self, amount: u128) {
        let mut guard = self.inner.lock().expect("balance cache mutex");
        if let Some((_, balance)) = guard.as_mut() {
            *balance = balance.saturating_sub(amount);
        }
    }

    fn clear(&self) {
        *self.inner.lock().expect("balance cache mutex") = None;
    }
}

#[derive(Clone)]
struct AppState {
    client: ChainClient,
    store: Arc<FaucetStore>,
    sign_lock: Arc<tokio::sync::Mutex<()>>,
    balance_cache: Arc<BalanceCache>,
    wallet_name: String,
    faucet_address: String,
    drip_base_units: u64,
    hot_wallet_reserve: u64,
    trust_proxy: bool,
}

fn failure(status: actix_web::http::StatusCode, message: impl Into<String>) -> HttpResponse {
    HttpResponse::build(status).json(FaucetResponse {
        success: false,
        message: Some(message.into()),
        tx_hash: None,
        amount: None,
    })
}

fn first_forwarded_hop(header: &str) -> Option<&str> {
    let hop = header.split(',').next()?.trim();
    if hop.is_empty() {
        None
    } else {
        Some(hop)
    }
}

fn client_ip(req: &HttpRequest, trust_proxy: bool) -> String {
    if trust_proxy {
        if let Some(xff) = req
            .headers()
            .get("X-Forwarded-For")
            .and_then(|value| value.to_str().ok())
        {
            if let Some(hop) = first_forwarded_hop(xff) {
                return hop.to_string();
            }
        }
    }
    req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn retry_after_secs(kind: LimitKind, now: DateTime<Utc>) -> u64 {
    match kind {
        LimitKind::AddressDaily => {
            let midnight = (now.date_naive() + ChronoDuration::days(1))
                .and_time(chrono::NaiveTime::MIN)
                .and_utc();
            midnight.signed_duration_since(now).num_seconds().max(1) as u64
        }
        LimitKind::IpHourly => {
            let ts = now.timestamp();
            let hour_end = ts.div_euclid(3600) * 3600 + 3600;
            (hour_end - ts).max(1) as u64
        }
    }
}

fn rate_limited(kind: LimitKind, now: DateTime<Utc>) -> HttpResponse {
    HttpResponse::TooManyRequests()
        .insert_header(("Retry-After", retry_after_secs(kind, now).to_string()))
        .json(FaucetResponse {
            success: false,
            message: Some("try again later".to_string()),
            tx_hash: None,
            amount: None,
        })
}

fn is_network_or_broadcast_failure(err: &EldError) -> bool {
    matches!(err, EldError::NetworkError { .. })
}

fn trust_proxy_from_env() -> bool {
    env::var(TRUST_PROXY_ENV).ok().as_deref() == Some("1")
}

fn faucet_has_room(balance: u128, drip_base_units: u64, hot_wallet_reserve: u64) -> bool {
    balance >= u128::from(drip_base_units) + u128::from(hot_wallet_reserve)
}

async fn faucet_balance(state: &AppState) -> Result<u128, EldError> {
    if let Some(balance) = state.balance_cache.get(BALANCE_CACHE_TTL) {
        return Ok(balance);
    }
    let balance = match state
        .client
        .get_account(state.faucet_address.clone())
        .await?
    {
        Some(account) => account.balance().amount(),
        None => 0,
    };
    state.balance_cache.set(balance);
    Ok(balance)
}

async fn request_tokens(
    req: HttpRequest,
    body: web::Json<FaucetRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if Address::parse_hex_str(&body.address).is_err() {
        return failure(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid address format",
        );
    }

    let ip = client_ip(&req, state.trust_proxy);
    let now = Utc::now();
    let reservation = match state.store.reserve(&body.address, &ip, now) {
        Ok(reservation) => reservation,
        Err(StoreError::Limit(kind)) => return rate_limited(kind, now),
        Err(e) => {
            error!("Faucet store reserve failed: {e}");
            return failure(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Error submitting tx",
            );
        }
    };

    let submitted = {
        let _sign = state.sign_lock.lock().await;
        match faucet_balance(&state).await {
            Ok(balance) => {
                if !faucet_has_room(balance, state.drip_base_units, state.hot_wallet_reserve) {
                    Err(None)
                } else {
                    match state
                        .client
                        .transfer(
                            state.wallet_name.clone(),
                            body.address.clone(),
                            u128::from(state.drip_base_units),
                        )
                        .await
                    {
                        Ok(submitted) => {
                            state
                                .balance_cache
                                .subtract(u128::from(state.drip_base_units));
                            Ok(submitted)
                        }
                        Err(e) => {
                            state.balance_cache.clear();
                            Err(Some(e))
                        }
                    }
                }
            }
            Err(e) => {
                state.balance_cache.clear();
                Err(Some(e))
            }
        }
    };

    match submitted {
        Ok(submitted) => {
            let tx_hash = submitted.tx_hash.to_string();
            if let Err(e) = state.store.commit(&reservation.address, &tx_hash) {
                error!("Faucet store commit failed: {e}");
            }
            info!(
                address = %reservation.address,
                tx_hash = %tx_hash,
                "Faucet transfer tx submitted"
            );
            HttpResponse::Ok().json(FaucetResponse {
                success: true,
                message: Some("tx submit".to_string()),
                tx_hash: Some(tx_hash),
                amount: Some(state.drip_base_units.to_string()),
            })
        }
        Err(None) => {
            if let Err(release_err) =
                state
                    .store
                    .release(&reservation.address, &reservation.ip, now)
            {
                error!("Faucet store release failed: {release_err}");
            }
            failure(
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "faucet empty",
            )
        }
        Err(Some(e)) => {
            if let Err(release_err) =
                state
                    .store
                    .release(&reservation.address, &reservation.ip, now)
            {
                error!("Faucet store release failed: {release_err}");
            }
            error!("Faucet transfer failed: {e}");
            if is_network_or_broadcast_failure(&e) {
                failure(
                    actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                    "node unavailable",
                )
            } else {
                failure(
                    actix_web::http::StatusCode::BAD_REQUEST,
                    "Error submitting tx",
                )
            }
        }
    }
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().body("OK")
}

fn io_err(err: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

fn spawn_purge_task(store: Arc<FaucetStore>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(3600));
        loop {
            ticker.tick().await;
            let cutoff = Utc::now() - ChronoDuration::days(2);
            if let Err(e) = store.purge_older_than(cutoff) {
                error!("Faucet store purge failed: {e}");
            }
        }
    });
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    logging::init_default_logging().map_err(io_err)?;

    info!("Starting faucet server");
    let faucet_config = FaucetConfig::load().map_err(io_err)?;
    let tendermint_url = faucet_config.node_url().map_err(io_err)?;
    let bind_addr = faucet_config.bind_addr();
    let trust_proxy = trust_proxy_from_env();
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
        trust_proxy,
        "Faucet config loaded"
    );

    let store = Arc::new(
        FaucetStore::open(
            &faucet_config.db_path,
            faucet_config.address_daily_drips,
            faucet_config.ip_hourly_requests,
        )
        .map_err(io_err)?,
    );
    spawn_purge_task(Arc::clone(&store));

    let client = ChainClient::with_wallets(
        faucet_config.to_client_config(),
        FeeConfig::default(),
        &faucet_config.wallet_path,
    )
    .map_err(io_err)?;

    let faucet_address = match client
        .get_wallet_by_name(faucet_config.wallet_name.clone())
        .await
    {
        Ok(Some(wallet)) => {
            info!(
                "Faucet wallet '{}' loaded from {}",
                faucet_config.wallet_name,
                faucet_config.wallet_path.display()
            );
            wallet.address.hex_with_prefix()
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
    };

    let state = AppState {
        client,
        store,
        sign_lock: Arc::new(tokio::sync::Mutex::new(())),
        balance_cache: Arc::new(BalanceCache::new()),
        wallet_name: faucet_config.wallet_name.clone(),
        faucet_address,
        drip_base_units: faucet_config.drip_base_units,
        hot_wallet_reserve: faucet_config.hot_wallet_reserve,
        trust_proxy,
    };

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(state.clone()))
            .app_data(web::JsonConfig::default().limit(JSON_BODY_LIMIT))
            .route(HEALTH_PATH, web::get().to(health_check))
            .route(REQUEST_PATH, web::post().to(request_tokens))
    })
    .bind(bind_addr)?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_forwarded_hop_takes_leftmost() {
        assert_eq!(
            first_forwarded_hop("203.0.113.1, 192.168.1.1"),
            Some("203.0.113.1")
        );
        assert_eq!(first_forwarded_hop("  10.0.0.2  "), Some("10.0.0.2"));
        assert_eq!(first_forwarded_hop("  , 10.0.0.2"), None);
        assert_eq!(first_forwarded_hop(""), None);
    }

    #[test]
    fn retry_after_address_daily_is_until_utc_midnight() {
        let now = DateTime::from_timestamp(1_699_996_800, 0).expect("timestamp");
        // 2023-11-14 21:20:00 UTC -> 2h 40m until 2023-11-15 00:00:00
        assert_eq!(
            retry_after_secs(LimitKind::AddressDaily, now),
            2 * 3600 + 40 * 60
        );
    }

    #[test]
    fn retry_after_ip_hourly_is_until_hour_end() {
        let now = DateTime::from_timestamp(1_699_996_800, 0).expect("timestamp");
        // 21:20:00 -> 2400 seconds until 22:00:00
        assert_eq!(retry_after_secs(LimitKind::IpHourly, now), 2400);
    }

    #[test]
    fn faucet_has_room_requires_drip_plus_reserve() {
        assert!(faucet_has_room(11_000, 1_000, 10_000));
        assert!(!faucet_has_room(10_999, 1_000, 10_000));
        assert!(!faucet_has_room(0, 1, 0));
        assert!(faucet_has_room(1, 1, 0));
    }
}
