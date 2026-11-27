use crate::store::AnalyticsStore;
use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

/// Same allow-all values as the Tendermint RPC listener.
const CORS_ALLOW_ORIGIN: &str = "*";
const CORS_ALLOW_METHODS: &str = "GET, POST, OPTIONS";
const CORS_ALLOW_HEADERS: &str = "Content-Type";

pub struct AppState {
    pub store: Arc<AnalyticsStore>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/analytics/usage/latest", get(latest))
        .route("/analytics/usage/history", get(history))
        .with_state(state)
        .layer(cors_layer())
}

fn cors_layer() -> CorsLayer {
    let methods = CORS_ALLOW_METHODS
        .split(',')
        .map(|part| Method::from_bytes(part.trim().as_bytes()).expect("cors method"))
        .collect::<Vec<_>>();
    let headers = CORS_ALLOW_HEADERS
        .split(',')
        .map(|part| header::HeaderName::from_bytes(part.trim().as_bytes()).expect("cors header"))
        .collect::<Vec<_>>();
    CorsLayer::new()
        .allow_origin(HeaderValue::from_static(CORS_ALLOW_ORIGIN))
        .allow_methods(methods)
        .allow_headers(headers)
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_app() -> (Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = AnalyticsStore::open(&dir.path().join("analytics.rocksdb")).unwrap();
        (
            router(Arc::new(AppState {
                store: Arc::new(store),
            })),
            dir,
        )
    }

    fn header_value(response: &axum::response::Response, name: header::HeaderName) -> String {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned()
    }

    #[tokio::test]
    async fn health_allows_any_origin() {
        let (app, _dir) = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header(header::ORIGIN, "https://example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header_value(&response, header::ACCESS_CONTROL_ALLOW_ORIGIN),
            CORS_ALLOW_ORIGIN
        );
    }

    #[tokio::test]
    async fn preflight_allows_any_origin() {
        let (app, _dir) = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/analytics/usage/latest")
                    .header(header::ORIGIN, "https://example.com")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header_value(&response, header::ACCESS_CONTROL_ALLOW_ORIGIN),
            CORS_ALLOW_ORIGIN
        );
        let methods: Vec<String> = header_value(&response, header::ACCESS_CONTROL_ALLOW_METHODS)
            .split(',')
            .map(|part| part.trim().to_owned())
            .collect();
        for method in CORS_ALLOW_METHODS.split(',').map(str::trim) {
            assert!(methods.iter().any(|got| got == method));
        }
        assert_eq!(
            header_value(&response, header::ACCESS_CONTROL_ALLOW_HEADERS).to_ascii_lowercase(),
            CORS_ALLOW_HEADERS.to_ascii_lowercase()
        );
    }
}
