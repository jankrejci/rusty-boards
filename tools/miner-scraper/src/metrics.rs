//! Prometheus metric line formatting.
//!
//! Provides helpers to format gauge metrics with labels and optional timestamps
//! in the Prometheus text exposition format.

use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
#[path = "tests/metrics.rs"]
mod tests;

/// Format a gauge metric with labels and a millisecond timestamp.
///
/// Labels are provided as a slice of `(key, value)` pairs. The timestamp is
/// the current wall-clock time in milliseconds since the Unix epoch.
pub fn gauge(name: &str, labels: &[(&str, &str)], value: f64) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let mut line = String::with_capacity(128);
    line.push_str(name);

    if !labels.is_empty() {
        line.push('{');
        let label_str: String = labels
            .iter()
            .map(|(key, val)| format!("{key}=\"{val}\""))
            .collect::<Vec<_>>()
            .join(",");
        line.push_str(&label_str);
        line.push('}');
    }

    let _ = write!(line, " {} {}", format_value(value), ts);
    line
}

/// Format a float value, using integer representation when possible.
fn format_value(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}
