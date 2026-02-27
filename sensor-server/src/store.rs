//! Per-port Prometheus metrics storage and line validation.
//!
//! Serial readers write validated metric batches per port. The HTTP handler
//! reads all ports and concatenates them into a single response.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

/// Validated Prometheus metric lines for a single serial port.
struct PortMetrics {
    lines: Vec<String>,
}

/// Thread-safe store of per-port Prometheus metrics.
///
/// Serial reader tasks write metrics for their port. The HTTP handler reads
/// all ports and concatenates them into a single response.
#[derive(Clone)]
pub struct MetricsStore {
    inner: Arc<RwLock<HashMap<String, PortMetrics>>>,
}

impl MetricsStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Replace all stored metrics for a port with new validated lines.
    pub async fn update(&self, port: &str, lines: Vec<String>) {
        let mut store = self.inner.write().await;
        store.insert(port.to_owned(), PortMetrics { lines });
    }

    /// Remove metrics for a port that is no longer connected.
    pub async fn remove(&self, port: &str) {
        let mut store = self.inner.write().await;
        store.remove(port);
    }

    /// Render all stored metrics into a single Prometheus-compatible response.
    pub async fn render(&self) -> String {
        let store = self.inner.read().await;
        let mut output = String::new();
        for metrics in store.values() {
            for line in &metrics.lines {
                output.push_str(line);
                output.push('\n');
            }
        }
        output
    }
}

/// Parsed components of a Prometheus metric line.
struct MetricParts<'a> {
    /// Everything up to and including the space before the value.
    prefix: &'a str,
    /// The numeric value token.
    value: &'a str,
}

/// Parse a metric line into prefix and value, skipping comments and empty lines.
///
/// Returns None for comments, empty lines, and unparseable lines.
fn parse_metric_parts(line: &str) -> Option<MetricParts<'_>> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let bytes = line.as_bytes();
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
        return None;
    }

    // Find the value separator: first space after the metric name and optional labels.
    let value_start = if let Some(brace) = line.find('{') {
        let close = line[brace..].find('}').map(|i| brace + i)?;
        line[close..].find(' ').map(|i| close + i + 1)?
    } else {
        line.find(' ').map(|i| i + 1)?
    };

    let prefix = &line[..value_start];
    let rest = &line[value_start..];

    let value = match rest.find(' ') {
        Some(i) => &rest[..i],
        None => rest,
    };

    if value.is_empty() {
        return None;
    }

    Some(MetricParts { prefix, value })
}

/// Check whether a line looks like a valid Prometheus metric.
///
/// Accepts lines matching: `metric_name[{labels}] value [timestamp]`
/// Also accepts comment lines (starting with #) and empty lines.
pub fn is_valid_metric_line(line: &str) -> bool {
    if line.is_empty() || line.starts_with('#') {
        return true;
    }

    let parts = match parse_metric_parts(line) {
        Some(p) => p,
        None => return false,
    };

    let first = parts.value.as_bytes()[0];
    first.is_ascii_digit()
        || first == b'+'
        || first == b'-'
        || parts.value.starts_with("NaN")
        || parts.value.starts_with("Inf")
        || parts.value.starts_with("inf")
}

/// Replace or append the timestamp on a metric line with the current wall-clock time.
///
/// Comments and empty lines are returned unchanged. For metric lines, the
/// firmware's uptime timestamp (if present) is replaced with epoch milliseconds.
pub fn stamp_metric_line(line: &str) -> String {
    if line.is_empty() || line.starts_with('#') {
        return line.to_owned();
    }

    let parts = match parse_metric_parts(line) {
        Some(p) => p,
        None => return line.to_owned(),
    };

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    format!("{}{} {now_ms}", parts.prefix, parts.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_metric_lines() {
        assert!(is_valid_metric_line(
            "temperature_celsius{sensor=\"0\"} 23.5 1708000000000"
        ));
        assert!(is_valid_metric_line("up 1"));
        assert!(is_valid_metric_line(
            "# HELP temperature_celsius Temperature in Celsius"
        ));
        assert!(is_valid_metric_line("# TYPE temperature_celsius gauge"));
        assert!(is_valid_metric_line(""));
        assert!(is_valid_metric_line("metric_name -1.5"));
        assert!(is_valid_metric_line("metric_name +Inf"));
        assert!(is_valid_metric_line("metric_name NaN"));
    }

    #[test]
    fn invalid_metric_lines() {
        assert!(!is_valid_metric_line("not a metric at all garbage"));
        assert!(!is_valid_metric_line("123_starts_with_digit 1"));
        assert!(!is_valid_metric_line("metric_name{unclosed 1"));
        assert!(!is_valid_metric_line("metric_name"));
        assert!(!is_valid_metric_line("metric_name "));
        assert!(!is_valid_metric_line("metric_name{label=\"v\"} "));
    }

    #[test]
    fn stamp_replaces_uptime_timestamp() {
        let stamped = stamp_metric_line("temperature_celsius{sensor=\"0\"} 23.5 12345");
        assert!(stamped.starts_with("temperature_celsius{sensor=\"0\"} 23.5 "));
        let ts: u128 = stamped.rsplit(' ').next().unwrap().parse().unwrap();
        assert!(ts > 1_700_000_000_000);
    }

    #[test]
    fn stamp_adds_timestamp_when_missing() {
        let stamped = stamp_metric_line("up 1");
        assert!(stamped.starts_with("up 1 "));
        let ts: u128 = stamped.rsplit(' ').next().unwrap().parse().unwrap();
        assert!(ts > 1_700_000_000_000);
    }

    #[test]
    fn stamp_preserves_comments() {
        assert_eq!(
            stamp_metric_line("# HELP temp Temperature"),
            "# HELP temp Temperature"
        );
        assert_eq!(stamp_metric_line(""), "");
    }

    #[test]
    fn stamp_with_labels_no_timestamp() {
        let stamped = stamp_metric_line("temp{sensor=\"0\"} 23.5");
        assert!(stamped.starts_with("temp{sensor=\"0\"} 23.5 "));
    }

    #[tokio::test]
    async fn store_update_and_render() {
        let store = MetricsStore::new();
        store.update("/dev/ttyACM0", vec!["up 1".to_owned()]).await;
        store
            .update("/dev/ttyACM1", vec!["temp 23.5".to_owned()])
            .await;

        let output = store.render().await;
        assert!(output.contains("up 1\n"));
        assert!(output.contains("temp 23.5\n"));
    }

    #[tokio::test]
    async fn store_remove() {
        let store = MetricsStore::new();
        store.update("/dev/ttyACM0", vec!["up 1".to_owned()]).await;
        store.remove("/dev/ttyACM0").await;

        let output = store.render().await;
        assert!(output.is_empty());
    }
}
