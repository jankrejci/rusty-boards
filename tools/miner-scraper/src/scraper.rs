//! Per-host miner scraper.
//!
//! Each miner target gets a `Scraper` that detects firmware, probes available
//! endpoints, and scrapes metrics on a per-tier schedule using `tokio::select!`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::Result;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

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
    metrics_sender: mpsc::Sender<(String, Vec<Metric>)>,
}

impl Scraper {
    /// Create a scraper for the given host.
    ///
    /// The scraper starts with no endpoints. Call `init()` to detect firmware
    /// and probe available endpoints before calling `run()`.
    pub fn new(host: IpAddr, metrics_sender: mpsc::Sender<(String, Vec<Metric>)>) -> Self {
        Self {
            host,
            endpoints: Vec::new(),
            metrics_sender,
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
    pub async fn run(&self, intervals: &ScrapingIntervals, shutdown: CancellationToken) {
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
            let scrape_result = tokio::select! {
                () = shutdown.cancelled() => return,
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
            if scrape_result.is_err() {
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

        self.metrics_sender
            .send((host_label, batch))
            .await
            .map_err(|_| anyhow::anyhow!("channel closed"))?;

        Ok(())
    }
}

/// Manages per-host scraper lifecycles based on config changes.
///
/// Watches the config channel for target list changes. Spawns a new scraper
/// task for each new target and cancels tasks for removed targets. Child tasks
/// are cancelled via `CancellationToken` for graceful shutdown.
pub struct ScraperManager {
    config_receiver: watch::Receiver<crate::config::Config>,
    metrics_sender: mpsc::Sender<(String, Vec<Metric>)>,
    tasks: HashMap<String, JoinHandle<()>>,
}

impl ScraperManager {
    pub fn new(
        config_receiver: watch::Receiver<crate::config::Config>,
        metrics_sender: mpsc::Sender<(String, Vec<Metric>)>,
    ) -> Self {
        Self {
            config_receiver,
            metrics_sender,
            tasks: HashMap::new(),
        }
    }

    pub async fn run(mut self, shutdown: CancellationToken) {
        loop {
            let config = self.config_receiver.borrow_and_update().clone();

            // Cancel tasks for removed targets.
            let stale: Vec<String> = self
                .tasks
                .keys()
                .filter(|host| !config.targets.contains(host))
                .cloned()
                .collect();
            for host in stale {
                if let Some(handle) = self.tasks.remove(&host) {
                    handle.abort();
                }
                let _ = self.metrics_sender.send((host.clone(), Vec::new())).await;
                log::info!("removed stale host {host}");
            }

            // Spawn a scraper for each new target.
            for target in &config.targets {
                if self.tasks.contains_key(target) {
                    continue;
                }
                let metrics_sender = self.metrics_sender.clone();
                let intervals = config.scraping_intervals.clone();
                let target_owned = target.clone();
                let token = shutdown.child_token();
                let handle = tokio::spawn(async move {
                    let host: IpAddr = match target_owned.parse() {
                        Ok(ip) => ip,
                        Err(err) => {
                            log::error!("{target_owned}: invalid IP address: {err}");
                            return;
                        }
                    };
                    let mut scraper = Scraper::new(host, metrics_sender);
                    if let Err(err) = scraper.init().await {
                        log::warn!("{host}: {err}");
                        return;
                    }
                    scraper.run(&intervals, token).await;
                });
                self.tasks.insert(target.clone(), handle);
            }

            tokio::select! {
                () = shutdown.cancelled() => {
                    log::info!("shutdown signal received, stopping scrapers");
                    break;
                }
                result = self.config_receiver.changed() => {
                    if result.is_err() {
                        log::info!("config channel closed, stopping scrapers");
                        break;
                    }
                }
            }
        }

        for (_, task) in self.tasks.drain() {
            let _ = task.await;
        }
    }
}
