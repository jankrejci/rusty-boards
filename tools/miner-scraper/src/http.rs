//! HTTP server for Prometheus metric scraping.
//!
//! Serves a single `GET /metrics` endpoint that returns all stored miner
//! metrics in Prometheus text exposition format.

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::store::StoreState;

/// Prometheus text exposition content type.
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

async fn metrics(State(handle): State<StoreState>) -> impl IntoResponse {
    let body = handle.render().await;
    ([(header::CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)], body)
}

pub fn router(handle: StoreState) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(handle)
}
