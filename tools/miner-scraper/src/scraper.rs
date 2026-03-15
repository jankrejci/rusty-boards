//! Per-host miner scraper.
//!
//! Each miner target gets a `Scraper` that detects firmware, probes available
//! endpoints, and scrapes metrics on a per-tier schedule using `tokio::select!`.

use std::net::IpAddr;
use std::time::Duration;

use anyhow::Result;

use tokio::sync::mpsc;

use crate::config::ScrapingIntervals;
use crate::endpoint::{self, Endpoint, Firmware, Response, ScrapeTier, ENDPOINTS};
use crate::metrics::{Metric, MetricBuilder};
use crate::parser::{JsonParser, Parser, PlainParser};

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
        match self.detect_firmware().await {
            Ok(firmware) => log::info!("{}: detected {firmware} firmware", self.host),
            Err(err) => log::warn!("{}: firmware detection failed: {err}", self.host),
        }

        self.probe_endpoints().await;

        if self.endpoints.is_empty() {
            anyhow::bail!("{}: no endpoints available", self.host);
        }

        Ok(())
    }

    /// Detect the firmware running on a miner by sending a stats command.
    async fn detect_firmware(&self) -> Result<Firmware> {
        let response = FW_DETECT_ENDPOINT.fetch(self.host).await?;
        Ok(Firmware::identify(&response))
    }

    /// Probe all known endpoints sequentially and keep the supported ones.
    ///
    /// Sends each endpoint command with a delay between probes to avoid
    /// overwhelming the miner.
    async fn probe_endpoints(&mut self) {
        for &endpoint in ENDPOINTS {
            match endpoint.fetch(self.host).await {
                Ok(ref response) if !endpoint::is_error(response) => {
                    log::info!("{}: {} supported", self.host, endpoint.command());
                    self.endpoints.push(endpoint);
                }
                Ok(_) => {
                    log::debug!("{}: {} returned error", self.host, endpoint.command());
                }
                Err(_) => {
                    log::debug!("{}: {} not supported", self.host, endpoint.command());
                }
            }
            tokio::time::sleep(PROBE_DELAY).await;
        }
        log::info!(
            "{}: probed {} endpoints: {:?}",
            self.host,
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
    pub async fn run(&self, intervals: &ScrapingIntervals) {
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

        let mut high_interval = tokio::time::interval(intervals.tier_high_secs);
        let mut mid_interval = tokio::time::interval(intervals.tier_mid_secs);
        let mut low_interval = tokio::time::interval(intervals.tier_low_secs);

        loop {
            let result = tokio::select! {
                _ = high_interval.tick(), if !high.is_empty() => {
                    self.scrape_tier(&high).await
                }
                _ = mid_interval.tick(), if !mid.is_empty() => {
                    self.scrape_tier(&mid).await
                }
                _ = low_interval.tick(), if !low.is_empty() => {
                    self.scrape_tier(&low).await
                }
            };
            if result.is_err() {
                return;
            }
        }
    }

    /// Scrape a set of endpoints and send the metrics through the channel.
    ///
    /// Returns an error if the channel is closed and the scraper should stop.
    async fn scrape_tier(&self, endpoints: &[Endpoint]) -> Result<()> {
        let host_label = self.host.to_string();
        let mut batch = Vec::new();

        for &endpoint in endpoints {
            let response = match endpoint.fetch(self.host).await {
                Ok(response) => response,
                Err(err) => {
                    log::warn!("{host_label}/{}: {err}", endpoint.command());
                    continue;
                }
            };

            let lines = match response {
                Response::Json(value) => JsonParser::new(value).parse(),
                Response::Text(text) => {
                    PlainParser::new(text, endpoint.command().to_string()).parse()
                }
            };

            let metrics: Vec<Metric> = lines
                .into_iter()
                .map(|line| {
                    MetricBuilder::default()
                        .name(line.name)
                        .label("host", &host_label)
                        .extend_labels(line.labels)
                        .value(line.value)
                        .build()
                        .expect("BUG: ParsedLine always has name and value")
                })
                .collect();

            batch.extend(metrics);
        }

        if batch.is_empty() {
            return Ok(());
        }

        self.tx
            .send((host_label, batch))
            .await
            .map_err(|_| anyhow::anyhow!("channel closed"))?;

        Ok(())
    }
}
