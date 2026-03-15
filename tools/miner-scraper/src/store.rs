//! Per-host Prometheus metrics storage.
//!
//! `Store` owns the channel receiver and writes incoming metrics. `StoreState`
//! is a lightweight read-only clone handed to the HTTP layer.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};

use crate::metrics::Metric;

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;

/// Read-only handle for the HTTP handler.
#[derive(Clone)]
pub struct StoreState {
    inner: Arc<RwLock<HashMap<String, Vec<Metric>>>>,
}

impl StoreState {
    /// Render all stored metrics into a single Prometheus-compatible response.
    pub async fn render(&self) -> String {
        let store = self.inner.read().await;
        let mut output = String::new();
        for metric in store.values().flatten() {
            use std::fmt::Write;
            let _ = writeln!(output, "{metric}");
        }
        output
    }

    /// Replace all stored metrics for a host.
    #[cfg(test)]
    pub async fn update(&self, host: &str, metrics: Vec<Metric>) {
        let mut store = self.inner.write().await;
        store.insert(host.to_owned(), metrics);
    }

    /// Remove metrics for a host that is no longer in the target list.
    #[cfg(test)]
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
}

/// Metric storage that receives batches from scrapers via a channel.
pub struct Store {
    handle: StoreState,
    receiver: mpsc::Receiver<(String, Vec<Metric>)>,
}

impl Store {
    pub fn new(receiver: mpsc::Receiver<(String, Vec<Metric>)>) -> Self {
        Self {
            handle: StoreState {
                inner: Arc::new(RwLock::new(HashMap::new())),
            },
            receiver,
        }
    }

    /// Return a read-only view for the HTTP handler.
    pub fn state(&self) -> StoreState {
        self.handle.clone()
    }

    /// Receive metrics from scrapers and write them to the store.
    ///
    /// Runs until the channel is closed (all senders dropped). An empty metrics
    /// vec removes the host from the store.
    pub async fn run(mut self) {
        while let Some((host, metrics)) = self.receiver.recv().await {
            let mut store = self.handle.inner.write().await;
            if metrics.is_empty() {
                store.remove(&host);
            } else {
                store.insert(host, metrics);
            }
        }
    }
}
