//! Per-host miner scraper.
//!
//! Each miner target gets a `Scraper` that detects firmware, probes available
//! endpoints, and scrapes metrics on a per-tier schedule using `tokio::select!`.

use std::net::IpAddr;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::config::TierIntervals;
use crate::endpoint::{self, Endpoint, Firmware, ScrapeTier, ENDPOINTS};
use crate::metrics::{Metric, MetricBuilder};
use crate::parser;

/// Delay between probing individual endpoints to avoid overwhelming the miner.
const PROBE_DELAY: Duration = Duration::from_secs(1);

/// Endpoint used for firmware detection.
const FW_DETECT_ENDPOINT: Endpoint = Endpoint::Cgminer("stats", ScrapeTier::High);

/// Per-host scraper that detects firmware and probes available endpoints.
pub struct Scraper {
    host: IpAddr,
    endpoints: Vec<Endpoint>,
    tx: mpsc::Sender<(String, Vec<Metric>)>,
}

impl Scraper {
    /// Create a scraper for the given host.
    ///
    /// The scraper starts with no endpoints. Call `init()` to detect firmware
    /// and probe available endpoints before calling `run()`.
    pub fn new(host: IpAddr, tx: mpsc::Sender<(String, Vec<Metric>)>) -> Self {
        Self {
            host,
            endpoints: Vec::new(),
            tx,
        }
    }

    /// Detect firmware and probe available endpoints.
    ///
    /// Returns an error if no endpoints are available after probing.
    pub async fn init(&mut self) -> Result<()> {
        let host_str = self.host.to_string();

        match self.detect_firmware().await {
            Ok(firmware) => log::info!("{host_str}: detected {firmware} firmware"),
            Err(err) => log::warn!("{host_str}: firmware detection failed: {err}"),
        }

        self.probe_endpoints().await;

        if self.endpoints.is_empty() {
            anyhow::bail!("{host_str}: no endpoints available");
        }

        Ok(())
    }

    /// Detect the firmware running on a miner by sending a stats command.
    async fn detect_firmware(&self) -> Result<Firmware> {
        let host_str = self.host.to_string();
        let stats = FW_DETECT_ENDPOINT.fetch(&host_str).await?;
        Ok(Firmware::identify(&stats))
    }

    /// Probe all known endpoints sequentially and keep the supported ones.
    ///
    /// Sends each endpoint command with a delay between probes to avoid
    /// overwhelming the miner.
    async fn probe_endpoints(&mut self) {
        let host_str = self.host.to_string();
        for &endpoint in ENDPOINTS {
            match endpoint.fetch(&host_str).await {
                Ok(ref resp) if !endpoint::is_error(resp) => {
                    self.endpoints.push(endpoint);
                }
                Ok(_) => {
                    log::debug!("{} returned error on {host_str}", endpoint.command());
                }
                Err(_) => {
                    log::debug!("{} not supported on {host_str}", endpoint.command());
                }
            }
            tokio::time::sleep(PROBE_DELAY).await;
        }
        log::info!(
            "{host_str}: probed {} endpoints: {:?}",
            self.endpoints.len(),
            self.endpoints
                .iter()
                .map(|e| e.command())
                .collect::<Vec<_>>()
        );
    }

    /// Run tier-based scrape loops until the channel is closed.
    ///
    /// Groups endpoints by tier and uses `tokio::select!` with per-tier
    /// intervals to scrape each tier at its configured rate. Only one tier
    /// scrapes at a time to avoid overwhelming the miner.
    pub async fn run(&self, tiers: &TierIntervals) {
        if self.endpoints.is_empty() {
            log::warn!("{}: no endpoints, nothing to scrape", self.host);
            return;
        }

        let high: Vec<Endpoint> = self
            .endpoints
            .iter()
            .copied()
            .filter(|endpoint| endpoint.tier() == ScrapeTier::High)
            .collect();
        let mid: Vec<Endpoint> = self
            .endpoints
            .iter()
            .copied()
            .filter(|endpoint| endpoint.tier() == ScrapeTier::Mid)
            .collect();
        let low: Vec<Endpoint> = self
            .endpoints
            .iter()
            .copied()
            .filter(|endpoint| endpoint.tier() == ScrapeTier::Low)
            .collect();

        let mut high_interval = tokio::time::interval(Duration::from_secs(tiers.high_secs));
        let mut mid_interval = tokio::time::interval(Duration::from_secs(tiers.mid_secs));
        let mut low_interval = tokio::time::interval(Duration::from_secs(tiers.low_secs));

        loop {
            tokio::select! {
                _ = high_interval.tick(), if !high.is_empty() => {
                    if !self.scrape_tier(&high).await {
                        return;
                    }
                }
                _ = mid_interval.tick(), if !mid.is_empty() => {
                    if !self.scrape_tier(&mid).await {
                        return;
                    }
                }
                _ = low_interval.tick(), if !low.is_empty() => {
                    if !self.scrape_tier(&low).await {
                        return;
                    }
                }
            }
        }
    }

    /// Scrape a set of endpoints and send the metrics through the channel.
    ///
    /// Returns `false` if the channel is closed and the scraper should stop.
    async fn scrape_tier(&self, endpoints: &[Endpoint]) -> bool {
        let host_str = self.host.to_string();
        let mut metrics = Vec::new();

        for &endpoint in endpoints {
            match endpoint.fetch(&host_str).await {
                Ok(mut resp) => {
                    let parsed = parser::parse_response(&mut resp);
                    for line in parsed {
                        // ParsedLine always has name and value.
                        let metric = MetricBuilder::default()
                            .name(line.name)
                            .label("host", &host_str)
                            .extend_labels(line.labels)
                            .value(line.value)
                            .build()
                            .expect("BUG: ParsedLine always has name and value");
                        metrics.push(metric);
                    }
                }
                Err(err) => {
                    log::warn!("{host_str}/{}: {err}", endpoint.command());
                }
            }
        }

        if metrics.is_empty() {
            return true;
        }

        self.tx.send((host_str, metrics)).await.is_ok()
    }
}
