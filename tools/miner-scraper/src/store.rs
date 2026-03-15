//! Per-host Prometheus metrics storage.
//!
//! Metrics are stored as `Vec<Metric>` per host address. The inner `RwLock`
//! allows concurrent reads from the HTTP handler while the receiver task
//! writes. `Arc` enables shared ownership between the receiver task, the
//! HTTP handler, and the scrape orchestrator.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};

use crate::metrics::Metric;

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;

/// Thread-safe per-host metric storage.
#[derive(Clone)]
pub struct Store {
    inner: Arc<RwLock<HashMap<String, Vec<Metric>>>>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Replace all stored metrics for a host.
    pub async fn update(&self, host: &str, metrics: Vec<Metric>) {
        let mut store = self.inner.write().await;
        store.insert(host.to_owned(), metrics);
    }

    /// Remove metrics for a host that is no longer in the target list.
    pub async fn remove(&self, host: &str) {
        let mut store = self.inner.write().await;
        store.remove(host);
    }

    /// Return all hosts currently in the store.
    #[cfg(test)]
    pub async fn hosts(&self) -> Vec<String> {
        let store = self.inner.read().await;
        store.keys().cloned().collect()
    }

    /// Render all stored metrics into a single Prometheus-compatible response.
    pub async fn render(&self) -> String {
        let store = self.inner.read().await;
        let mut output = String::new();
        for metrics in store.values() {
            for metric in metrics {
                use std::fmt::Write;
                let _ = writeln!(output, "{metric}");
            }
        }
        output
    }

    /// Receive metrics from scrapers and write them to the store.
    ///
    /// Runs until the channel is closed (all senders dropped).
    pub async fn run(self, mut rx: mpsc::Receiver<(String, Vec<Metric>)>) {
        while let Some((host, metrics)) = rx.recv().await {
            self.update(&host, metrics).await;
        }
    }
}
