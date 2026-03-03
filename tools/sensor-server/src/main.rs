//! Sensor server entry point.
//!
//! Parses CLI arguments, starts the HTTP server, discovery loop, and metric
//! drain task, then waits for shutdown.

use std::net::SocketAddr;

use clap::Parser;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod discovery;
mod http;
mod serial;
mod store;

/// Channel buffer size for metric batches from serial readers.
const BATCH_CHANNEL_SIZE: usize = 64;

#[derive(Parser)]
#[command(about = "Bridge serial sensor metrics to Prometheus over HTTP")]
struct Args {
    /// Address to listen on.
    #[arg(long, default_value = "0.0.0.0:8888")]
    listen: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let store = store::MetricsStore::new();
    let token = CancellationToken::new();

    let (tx, rx) = mpsc::channel::<serial::MetricBatch>(BATCH_CHANNEL_SIZE);

    // Drain metric batches from serial readers into the store.
    let drain_store = store.clone();
    let drain_handle = tokio::spawn(async move {
        drain_batches(rx, drain_store).await;
    });

    // Discover serial ports and manage reader lifecycle. tx is moved here;
    // when this task exits, the sender drops, which terminates drain_batches.
    let discovery_store = store.clone();
    let discovery_token = token.clone();
    let discovery_handle = tokio::spawn(async move {
        discovery::run(tx, discovery_store, discovery_token).await;
    });

    let listener = TcpListener::bind(args.listen).await?;
    log::info!("listening on {}", args.listen);

    // Shutdown sequence: ctrl-c cancels the token, which stops discovery.
    // Discovery dropping tx causes drain_batches to exit via recv() returning None.
    let shutdown_token = token.clone();
    let router = http::router(store);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            shutdown_token.cancel();
        })
        .await?;

    // Wait for background tasks and log panics.
    if let Err(e) = discovery_handle.await {
        log::error!("discovery task panicked: {}", e);
    }
    if let Err(e) = drain_handle.await {
        log::error!("drain task panicked: {}", e);
    }

    log::info!("shutting down");
    Ok(())
}

/// Receive metric batches from serial readers and update the store.
async fn drain_batches(mut rx: mpsc::Receiver<serial::MetricBatch>, store: store::MetricsStore) {
    while let Some(batch) = rx.recv().await {
        store.update(&batch.port, batch.lines).await;
    }
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        log::error!("failed to install ctrl-c handler: {}", e);
    }
}
