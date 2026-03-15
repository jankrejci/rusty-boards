//! Configuration file parsing and hot reload.
//!
//! Reads a TOML config file specifying the listen address, target miner IPs,
//! and scrape tier intervals. Watches the file with inotify for live changes.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use inotify::{Inotify, WatchMask};
use serde::Deserialize;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;

const DEFAULT_HIGH_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_MID_INTERVAL: Duration = Duration::from_secs(10);
const DEFAULT_LOW_INTERVAL: Duration = Duration::from_secs(60);

fn default_high_interval() -> Duration {
    DEFAULT_HIGH_INTERVAL
}

fn default_mid_interval() -> Duration {
    DEFAULT_MID_INTERVAL
}

fn default_low_interval() -> Duration {
    DEFAULT_LOW_INTERVAL
}

/// Deserialize a u64 seconds value into Duration.
fn deserialize_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let secs: u64 = serde::Deserialize::deserialize(deserializer)?;
    Ok(Duration::from_secs(secs))
}

/// Scrape intervals per tier.
///
/// Controls how often each tier of endpoints is polled. High tier covers
/// real-time data, mid tier covers aggregated data, and low tier covers
/// stable configuration.
#[derive(Debug, Clone, Deserialize)]
// User-specified naming convention for config file clarity.
#[allow(clippy::struct_field_names)]
pub struct ScrapingIntervals {
    #[serde(
        default = "default_high_interval",
        deserialize_with = "deserialize_secs"
    )]
    pub tier_high_secs: Duration,

    #[serde(
        default = "default_mid_interval",
        deserialize_with = "deserialize_secs"
    )]
    pub tier_mid_secs: Duration,

    #[serde(
        default = "default_low_interval",
        deserialize_with = "deserialize_secs"
    )]
    pub tier_low_secs: Duration,
}

impl Default for ScrapingIntervals {
    fn default() -> Self {
        Self {
            tier_high_secs: DEFAULT_HIGH_INTERVAL,
            tier_mid_secs: DEFAULT_MID_INTERVAL,
            tier_low_secs: DEFAULT_LOW_INTERVAL,
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
    pub scraping_intervals: ScrapingIntervals,
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
            scraping_intervals: ScrapingIntervals::default(),
        }
    }
}

/// Path-aware config file handle for loading and watching.
pub struct ConfigFile {
    path: PathBuf,
    sender: watch::Sender<Config>,
}

/// Buffer size for inotify event reads.
const INOTIFY_BUF_SIZE: usize = 256;

impl ConfigFile {
    pub fn new(path: PathBuf, sender: watch::Sender<Config>) -> Self {
        Self { path, sender }
    }

    /// Load configuration from the file.
    pub fn load(&self) -> anyhow::Result<Config> {
        let contents = std::fs::read_to_string(&self.path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Publish a config to all subscribers.
    pub fn publish(&self, config: Config) {
        self.sender.send_replace(config);
    }

    /// Watch the config file for changes and send updates through the channel.
    ///
    /// Uses inotify `CLOSE_WRITE` to detect when editors finish writing the file.
    /// On inotify failure, logs a warning and waits for shutdown.
    pub async fn watch(self, shutdown: CancellationToken) {
        let inotify = match Inotify::init() {
            Ok(i) => i,
            Err(e) => {
                log::warn!("failed to init inotify for config watch: {e}");
                shutdown.cancelled().await;
                return;
            }
        };

        if let Err(e) = inotify.watches().add(&self.path, WatchMask::CLOSE_WRITE) {
            log::warn!("failed to watch config file: {e}");
            shutdown.cancelled().await;
            return;
        }

        log::info!("watching {} for changes", self.path.display());

        let mut stream = match inotify.into_event_stream([0u8; INOTIFY_BUF_SIZE]) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("failed to create inotify event stream: {e}");
                shutdown.cancelled().await;
                return;
            }
        };

        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                event = stream.next() => {
                    match event {
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

                    match self.load() {
                        Ok(new_config) => {
                            log::info!("config reloaded from {}", self.path.display());
                            let _ = self.sender.send(new_config);
                        }
                        Err(e) => {
                            log::warn!("failed to reload config: {e}");
                        }
                    }
                }
            }
        }
    }
}
