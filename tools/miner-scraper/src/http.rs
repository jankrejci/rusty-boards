//! HTTP server for Prometheus metric scraping.
//!
//! Serves a single `GET /metrics` endpoint that returns all stored miner
//! metrics in Prometheus text exposition format.

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::store::Store;

/// Prometheus text exposition content type.
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

async fn metrics(State(store): State<Store>) -> impl IntoResponse {
    let body = store.render().await;
    ([(header::CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)], body)
}

pub fn router(store: Store) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(store)
}
