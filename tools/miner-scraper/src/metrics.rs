//! Prometheus metric formatting.
//!
//! Provides a `Metric` struct with `Display` for prometheus text exposition
//! format, and a builder for constructing metrics from parsed data.

use std::collections::HashMap;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
#[path = "tests/metrics.rs"]
mod tests;

/// Prometheus gauge metric with labels and timestamp.
pub struct Metric {
    pub name: String,
    pub labels: HashMap<String, String>,
    pub value: f64,
    pub timestamp_ms: u128,
}

impl fmt::Display for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;

        if !self.labels.is_empty() {
            write!(f, "{{")?;
            let mut first = true;
            // Sort labels for deterministic output.
            let mut pairs: Vec<_> = self.labels.iter().collect();
            pairs.sort_by_key(|(key, _)| *key);
            for (key, val) in pairs {
                if !first {
                    write!(f, ",")?;
                }
                write!(f, "{key}=\"{val}\"")?;
                first = false;
            }
            write!(f, "}}")?;
        }

        // Prometheus convention: emit integers without decimal point.
        if self.value.fract() == 0.0 && self.value.is_finite() {
            write!(f, " {:.0} {}", self.value, self.timestamp_ms)
        } else {
            write!(f, " {} {}", self.value, self.timestamp_ms)
        }
    }
}

/// Builder for constructing `Metric` instances.
///
/// Collects name, labels, and value. Returns `None` from `build()` if name
/// or value is missing.
#[derive(Default)]
pub struct MetricBuilder {
    name: Option<String>,
    labels: HashMap<String, String>,
    val: Option<f64>,
}

impl MetricBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn extend_labels(mut self, labels: Vec<(String, String)>) -> Self {
        self.labels.extend(labels);
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.val = Some(value);
        self
    }

    pub fn build(self) -> Option<Metric> {
        let name = self.name?;
        let val = self.val?;
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        Some(Metric {
            name,
            labels: self.labels,
            value: val,
            timestamp_ms,
        })
    }
}
