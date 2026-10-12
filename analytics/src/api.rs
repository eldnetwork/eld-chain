use crate::store::AnalyticsStore;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

pub struct AppState {
    pub store: Arc<AnalyticsStore>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/analytics/disk/latest", get(latest))
        .route("/analytics/disk/history", get(history))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    StatusCode::OK
}

async fn latest(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.store.latest() {
        Ok(run) => Json(run).into_response(),
        Err(e) => {
            tracing::error!("latest failed: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct HistoryParams {
    #[serde(default = "default_history_limit")]
    pub limit: usize,
}

fn default_history_limit() -> usize {
    24
}

async fn history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HistoryParams>,
) -> impl IntoResponse {
    let limit = params.limit.clamp(1, 168);
    match state.store.history(limit) {
        Ok(runs) => Json(runs).into_response(),
        Err(e) => {
            tracing::error!("history failed: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
