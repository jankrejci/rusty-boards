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

/// Maximum number of pending metric batches from scrapers to the store.
const METRICS_CHANNEL_SIZE: usize = 256;

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

    let (config_tx, config_rx) = watch::channel(config::Config::default());
    let (metrics_tx, metrics_rx) = mpsc::channel(METRICS_CHANNEL_SIZE);

    let config_file = config::ConfigFile::new(args.config, config_tx);
    let mut config = config_file.load().unwrap_or_else(|e| {
        log::warn!("failed to load config: {e}, using defaults");
        config::Config::default()
    });

    // Hierarchy: defaults -> config -> cli.
    let mut listen: SocketAddr = config.listen.parse()?;
    if let Some(ip) = args.ip {
        listen.set_ip(ip);
    }
    if !args.targets.is_empty() {
        config.targets = args.targets;
    }
    config_file.publish(config);

    // Bind the listener early so we fail fast if the port is in use.
    let listener = TcpListener::bind(listen)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind {listen}: {e}"))?;
    log::info!("listening on {listen}");

    let mut tasks = Vec::new();

    let store = store::Store::new(metrics_rx);
    let state = store.state();
    tasks.push(tokio::spawn(async move { store.run().await }));

    tasks.push(tokio::spawn(async move { config_file.watch().await }));

    let manager = scraper::ScraperManager::new(config_rx, metrics_tx);
    tasks.push(tokio::spawn(async move { manager.run().await }));

    let router = http::router(state);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    for task in &tasks {
        task.abort();
    }

    log::info!("shutting down");
    Ok(())
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        log::error!("failed to install ctrl-c handler: {e}");
    }
}
