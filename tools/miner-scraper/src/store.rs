//! Per-host Prometheus metrics storage.
//!
//! Scrape tasks write metric lines per miner host. The HTTP handler reads all
//! hosts and concatenates them into a single response.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;

/// Thread-safe store of per-host Prometheus metric lines.
#[derive(Clone)]
pub struct MetricsStore {
    inner: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl MetricsStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Replace all stored metrics for a host with new lines.
    pub async fn update(&self, host: &str, lines: Vec<String>) {
        let mut store = self.inner.write().await;
        store.insert(host.to_owned(), lines);
    }

    /// Remove metrics for a host that is no longer in the target list.
    pub async fn remove(&self, host: &str) {
        let mut store = self.inner.write().await;
        store.remove(host);
    }

    /// Return all hosts currently in the store.
    pub async fn hosts(&self) -> Vec<String> {
        let store = self.inner.read().await;
        store.keys().cloned().collect()
    }

    /// Render all stored metrics into a single Prometheus-compatible response.
    pub async fn render(&self) -> String {
        let store = self.inner.read().await;
        let mut output = String::new();
        for lines in store.values() {
            for line in lines {
                output.push_str(line);
                output.push('\n');
            }
        }
        output
    }
}
