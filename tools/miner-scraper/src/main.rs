//! Miner scraper entry point.
//!
//! Parses CLI arguments, starts the HTTP server, config watcher, and per-host
//! scraper tasks, then waits for shutdown.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};

mod config;
mod endpoint;
mod http;
mod metrics;
mod parser;
mod scraper;
mod store;

#[derive(Parser)]
#[command(about = "Scrape Bitcoin mining hardware metrics for Prometheus")]
struct Args {
    /// Path to TOML config file.
    #[arg(long, default_value = "/etc/miner-scraper/config.toml")]
    config: PathBuf,

    /// IP address to listen on. Overrides the config file.
    #[arg(long)]
    ip: Option<IpAddr>,

    /// Target miner IPs to scrape. Overrides the config file.
    #[arg(long, num_args = 1..)]
    targets: Vec<String>,
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use env_logger::fmt::style::AnsiColor;
            use std::io::Write;
            let level = record.level();
            let style = buf.default_level_style(level).fg_color(Some(match level {
                log::Level::Error => AnsiColor::Red.into(),
                log::Level::Warn => AnsiColor::Yellow.into(),
                log::Level::Info => AnsiColor::Green.into(),
                log::Level::Debug => AnsiColor::Blue.into(),
                log::Level::Trace => AnsiColor::Cyan.into(),
            }));
            let dim =
                env_logger::fmt::style::Style::new().fg_color(Some(AnsiColor::BrightBlack.into()));
            writeln!(
                buf,
                "{dim}{}{dim:#} {style}{:5}{style:#} {dim}{}{dim:#} {}",
                buf.timestamp(),
                level,
                record.module_path().unwrap_or(""),
                record.args()
            )
        })
        .init();

    if let Err(e) = run().await {
        log::error!("{e:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut cfg = config::Config::load(&args.config).unwrap_or_else(|e| {
        log::warn!(
            "config file {}: {}, using defaults",
            args.config.display(),
            e
        );
        config::Config::default()
    });

    // Hierarchy: defaults -> config -> cli.
    let mut listen: SocketAddr = cfg.listen.parse()?;
    if let Some(ip) = args.ip {
        listen.set_ip(ip);
    }
    if !args.targets.is_empty() {
        cfg.targets = args.targets;
    }

    // Bind the listener early so we fail fast if the port is in use.
    let listener = TcpListener::bind(listen)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind {listen}: {e}"))?;
    log::info!("listening on {listen}");

    let (config_tx, config_rx) = watch::channel(cfg);

    // Channel for metric batches from scrapers to the store.
    let (metrics_tx, metrics_rx) = mpsc::channel(256);

    let store = store::Store::new(metrics_rx);
    let handle = store.handle();

    // Receive metrics from scrapers and write to store.
    let store_handle = tokio::spawn(async move {
        store.run().await;
    });

    // Watch config file for hot reload.
    let config_path = args.config;
    let watcher_tx = config_tx.clone();
    let watcher_handle = tokio::spawn(async move {
        config::watch_config(config_path, watcher_tx).await;
    });

    // Manage per-host scrapers: spawn on new targets, cancel on removed ones.
    let manager = scraper::ScraperManager::new(config_rx, metrics_tx);
    let scrape_handle = tokio::spawn(async move { manager.run().await });

    let router = http::router(handle);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Drop the config sender to signal the scrape manager to stop.
    drop(config_tx);
    watcher_handle.abort();
    if let Err(e) = scrape_handle.await {
        log::error!("scrape manager panicked: {e}");
    }
    store_handle.abort();

    log::info!("shutting down");
    Ok(())
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        log::error!("failed to install ctrl-c handler: {e}");
    }
}
