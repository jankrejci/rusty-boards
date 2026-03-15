//! Configuration file parsing and hot reload.
//!
//! Reads a TOML config file specifying the listen address, target miner IPs,
//! and scrape tier intervals. Watches the file with inotify for live changes.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use inotify::{Inotify, WatchMask};
use serde::Deserialize;
use tokio::sync::watch;

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;

const DEFAULT_HIGH_INTERVAL_SECS: u64 = 1;
const DEFAULT_MID_INTERVAL_SECS: u64 = 10;
const DEFAULT_LOW_INTERVAL_SECS: u64 = 60;

fn default_high_interval() -> u64 {
    DEFAULT_HIGH_INTERVAL_SECS
}

fn default_mid_interval() -> u64 {
    DEFAULT_MID_INTERVAL_SECS
}

fn default_low_interval() -> u64 {
    DEFAULT_LOW_INTERVAL_SECS
}

/// Scrape intervals per tier.
///
/// Controls how often each tier of endpoints is polled. High tier covers
/// real-time data, mid tier covers aggregated data, and low tier covers
/// stable configuration.
#[derive(Debug, Clone, Deserialize)]
// Suffix makes units explicit in config file.
#[allow(clippy::struct_field_names)]
pub struct TierIntervals {
    #[serde(default = "default_high_interval")]
    pub high_secs: u64,

    #[serde(default = "default_mid_interval")]
    pub mid_secs: u64,

    #[serde(default = "default_low_interval")]
    pub low_secs: u64,
}

impl Default for TierIntervals {
    fn default() -> Self {
        Self {
            high_secs: DEFAULT_HIGH_INTERVAL_SECS,
            mid_secs: DEFAULT_MID_INTERVAL_SECS,
            low_secs: DEFAULT_LOW_INTERVAL_SECS,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_listen")]
    pub listen: String,

    #[serde(default)]
    pub targets: Vec<String>,

    #[serde(default)]
    pub tiers: TierIntervals,
}

pub const DEFAULT_IP: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8889;

fn default_listen() -> String {
    format!("{DEFAULT_IP}:{DEFAULT_PORT}")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            targets: Vec::new(),
            tiers: TierIntervals::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}

/// Buffer size for inotify event reads.
const INOTIFY_BUF_SIZE: usize = 256;

/// Watch a config file for changes and send updates through the watch channel.
///
/// Uses inotify `CLOSE_WRITE` to detect when editors finish writing the file.
/// On inotify failure, logs a warning and returns without watching.
pub async fn watch_config(path: PathBuf, tx: watch::Sender<Config>) {
    let inotify = match Inotify::init() {
        Ok(i) => i,
        Err(e) => {
            log::warn!("failed to init inotify for config watch: {e}");
            // Park forever so the caller does not need to handle None.
            std::future::pending::<()>().await;
            return;
        }
    };

    if let Err(e) = inotify.watches().add(&path, WatchMask::CLOSE_WRITE) {
        log::warn!("failed to watch config file: {e}");
        std::future::pending::<()>().await;
        return;
    }

    log::info!("watching {} for changes", path.display());

    let mut stream = match inotify.into_event_stream([0u8; INOTIFY_BUF_SIZE]) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("failed to create inotify event stream: {e}");
            std::future::pending::<()>().await;
            return;
        }
    };

    loop {
        match stream.next().await {
            Some(Ok(_event)) => {}
            Some(Err(e)) => {
                log::warn!("inotify error: {e}");
                return;
            }
            None => {
                log::warn!("inotify stream ended, stopping config watch");
                return;
            }
        }

        match Config::load(&path) {
            Ok(new_config) => {
                log::info!("config reloaded from {}", path.display());
                let _ = tx.send(new_config);
            }
            Err(e) => {
                log::warn!("failed to reload config: {e}");
            }
        }
    }
}
