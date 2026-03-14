//! Per-target scrape loops.
//!
//! Each target gets an independent scrape loop that detects firmware on first
//! contact and caches the result. The main function manages loop lifecycle:
//! spawning new loops when targets appear and cancelling them when removed.

use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::cgminer::{self, Firmware};
use crate::config::Config;
use crate::store::MetricsStore;

/// Number of consecutive failures before clearing the firmware cache for a host.
const MAX_FAILURES: u32 = 3;

/// Timeout for a single host scrape.
/// Prevents slow miners from delaying the next scrape cycle.
const SCRAPE_TIMEOUT: Duration = Duration::from_secs(10);

/// Manage per-target scrape loops, spawning and cancelling as the config changes.
pub async fn run(mut config_rx: watch::Receiver<Config>, store: MetricsStore) {
    let mut tasks: HashMap<String, JoinHandle<()>> = HashMap::new();

    loop {
        let config = config_rx.borrow_and_update().clone();

        // Cancel tasks for removed targets.
        let stale: Vec<String> = tasks
            .keys()
            .filter(|host| !config.targets.contains(host))
            .cloned()
            .collect();
        for host in stale {
            if let Some(handle) = tasks.remove(&host) {
                handle.abort();
            }
            store.remove(&host).await;
            log::info!("removed stale host {host}");
        }

        // Spawn loops for new targets.
        for target in &config.targets {
            if !tasks.contains_key(target) {
                let handle = tokio::spawn(scrape_loop(
                    target.clone(),
                    config_rx.clone(),
                    store.clone(),
                ));
                tasks.insert(target.clone(), handle);
            }
        }

        if config_rx.changed().await.is_err() {
            log::info!("config channel closed, stopping scrape loops");
            break;
        }
    }

    for (_, handle) in tasks {
        handle.abort();
    }
}

/// Independent scrape loop for a single target.
async fn scrape_loop(host: String, config_rx: watch::Receiver<Config>, store: MetricsStore) {
    let mut firmware: Option<Firmware> = None;
    let mut failure_count: u32 = 0;

    loop {
        let interval = Duration::from_secs(config_rx.borrow().scrape_interval_secs);

        match scrape_host(&host, firmware).await {
            Ok((fw, lines)) => {
                firmware = Some(fw);
                failure_count = 0;
                store.update(&host, lines).await;
            }
            Err(err) => {
                log::warn!("scrape {host} failed: {err}");
                failure_count += 1;
                if failure_count >= MAX_FAILURES {
                    log::info!("clearing firmware cache for {host} after {failure_count} failures");
                    firmware = None;
                    failure_count = 0;
                }
            }
        }

        tokio::time::sleep(interval).await;
    }
}

/// Scrape a single host, detecting firmware if not cached.
async fn scrape_host(
    host: &str,
    cached_fw: Option<Firmware>,
) -> anyhow::Result<(Firmware, Vec<String>)> {
    tokio::time::timeout(SCRAPE_TIMEOUT, async {
        let fw = match cached_fw {
            Some(fw) => fw,
            None => {
                let detected = Firmware::detect(host).await;
                log::info!("detected {detected} firmware on {host}");
                detected
            }
        };

        let lines = cgminer::scrape(host, fw).await?;
        Ok((fw, lines))
    })
    .await
    .map_err(|_| anyhow::anyhow!("scrape timeout for {host}"))?
}
