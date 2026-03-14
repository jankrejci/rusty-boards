//! Async TCP client and generic response parser for the cgminer socket API.
//!
//! Cgminer listens on TCP port 4028 and accepts JSON commands. Responses are
//! JSON terminated by a NUL byte.
//!
//! The parsing pipeline converts arbitrary cgminer JSON responses into
//! Prometheus metric lines: preprocess dash-separated strings into arrays,
//! then parse each field name and value into labeled metrics.

use std::time::Duration;

use anyhow::Result;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::metrics;

/// Default cgminer API port.
pub const DEFAULT_PORT: u16 = 4028;

/// Timeout for establishing a TCP connection to a miner.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for reading a complete response from a miner.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Known firmware types for Bitcoin mining hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firmware {
    Stock,
    LuxOS,
    Vnish,
    Braiins,
    Mara,
}

impl std::fmt::Display for Firmware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Firmware::Stock => write!(f, "stock"),
            Firmware::LuxOS => write!(f, "luxos"),
            Firmware::Vnish => write!(f, "vnish"),
            Firmware::Braiins => write!(f, "braiins"),
            Firmware::Mara => write!(f, "mara"),
        }
    }
}

impl Firmware {
    /// Return the cgminer commands to scrape for this firmware.
    pub fn commands(&self) -> &[&str] {
        match self {
            Firmware::Braiins => &["temps", "fans", "summary", "devs", "devdetails"],
            Firmware::LuxOS => &["stats", "temps", "fans", "power"],
            Firmware::Stock | Firmware::Vnish | Firmware::Mara => &["stats", "summary"],
        }
    }

    /// Determine firmware from stats response.
    ///
    /// Checks the STATUS Description field for `BraiinsOS`, `LuxOS`, and MARA
    /// identifiers. Falls back to the STATS Type field for Vnish. Returns
    /// stock firmware if nothing matches.
    fn identify(stats: &serde_json::Value) -> Firmware {
        let description = stats
            .pointer("/STATUS/0/Description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let type_field = stats
            .pointer("/STATS/0/Type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match (description, type_field) {
            (desc, _) if desc.contains("BOSer") => Firmware::Braiins,
            (desc, _) if desc.contains("LUXminer") => Firmware::LuxOS,
            (desc, _) if desc.contains("kaonsu") => Firmware::Mara,
            (_, typ) if typ.contains("(Vnish") => Firmware::Vnish,
            _ => Firmware::Stock,
        }
    }

    /// Detect the firmware running on a miner via cgminer API.
    ///
    /// Sends stats command and delegates to `identify()` for the actual
    /// firmware classification.
    pub async fn detect(host: &str) -> Self {
        let stats = command(host, DEFAULT_PORT, "stats").await.ok();
        match stats {
            Some(ref resp) => Self::identify(resp),
            None => Firmware::Stock,
        }
    }
}

/// Scrape metrics from a miner using the specified firmware commands.
pub async fn scrape(host: &str, fw: Firmware) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for cmd in fw.commands() {
        let mut resp = command(host, DEFAULT_PORT, cmd).await?;
        lines.extend(parse_response(host, &mut resp));
    }
    Ok(lines)
}

/// Parsed field name from a cgminer JSON key.
#[derive(Debug, PartialEq)]
pub enum FieldName {
    Plain(String),
    Indexed { name: String, index: u32 },
    Fan { index: u32 },
}

impl FieldName {
    /// Parse a raw JSON key into a structured field name.
    ///
    /// Normalizes the key to lowercase, replaces dots and spaces with
    /// underscores, then strips trailing digits as the board or fan index.
    pub fn parse(key: &str) -> Self {
        let normalized: String = key
            .to_lowercase()
            .replace(['.', ' '], "_")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let normalized = normalized.trim_end_matches('_').to_string();

        // Find where trailing digits begin by locating the last non-digit character.
        let digit_start = normalized
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |i| i + 1);

        if digit_start == 0 || digit_start >= normalized.len() {
            return FieldName::Plain(normalized);
        }

        let index: u32 = match normalized[digit_start..].parse() {
            Ok(i) => i,
            Err(_) => return FieldName::Plain(normalized),
        };

        let name = normalized[..digit_start].trim_end_matches('_');
        if name.is_empty() {
            return FieldName::Plain(normalized);
        }

        if name == "fan" {
            FieldName::Fan { index }
        } else {
            FieldName::Indexed {
                name: name.to_string(),
                index,
            }
        }
    }
}

/// Parsed field value from a cgminer JSON value.
#[derive(Debug, PartialEq)]
pub enum FieldValue {
    Number(f64),
    IndexedArray(Vec<f64>),
}

impl FieldValue {
    /// Parse a JSON value into a metric value.
    ///
    /// Numbers and numeric strings become scalars. Arrays of numbers become
    /// indexed arrays. Everything else is ignored.
    pub fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => n.as_f64().map(FieldValue::Number),
            Value::Array(arr) => {
                let nums: Vec<f64> = arr.iter().filter_map(serde_json::Value::as_f64).collect();
                if nums.len() == arr.len() && !nums.is_empty() {
                    Some(FieldValue::IndexedArray(nums))
                } else {
                    None
                }
            }
            Value::String(s) => s.parse::<f64>().ok().map(FieldValue::Number),
            _ => None,
        }
    }
}

/// Convert dash-separated numeric strings to JSON arrays in place.
///
/// Modifies an object so that values like `"43-58-65-63"` become
/// `[43, 58, 65, 63]`. Non-numeric or single-element strings are left alone.
fn preprocess(obj: &mut serde_json::Map<String, Value>) {
    for value in obj.values_mut() {
        let arr = match value {
            Value::String(s) => parse_dash_array(s),
            _ => continue,
        };
        let Some(arr) = arr else {
            continue;
        };
        *value = Value::Array(arr);
    }
}

fn parse_dash_array(s: &str) -> Option<Vec<Value>> {
    if !s.contains('-') {
        return None;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() < 2 {
        return None;
    }
    let nums: Vec<Value> = parts
        .iter()
        .filter_map(|p| p.parse::<f64>().ok())
        .map(Value::from)
        .collect();
    if nums.len() != parts.len() {
        return None;
    }
    Some(nums)
}

/// Parse a cgminer JSON response into Prometheus metric lines.
///
/// Iterates all data sections in the response (skipping STATUS and id),
/// preprocesses dash-separated strings into arrays, then emits each numeric
/// field as a gauge metric. The section name is lowercased and prepended to
/// each metric name (e.g. FANS.RPM becomes `fans_rpm`).
pub fn parse_response(host: &str, response: &mut Value) -> Vec<String> {
    let Some(obj) = response.as_object_mut() else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    for (section, value) in obj {
        if section == "STATUS" || section == "id" {
            continue;
        }

        let prefix = section.to_lowercase();
        let Some(entries) = value.as_array_mut() else {
            continue;
        };

        for entry in entries {
            lines.extend(parse_entry(host, &prefix, entry));
        }
    }
    lines
}

fn parse_entry(host: &str, prefix: &str, entry: &mut Value) -> Vec<String> {
    let Some(entry_obj) = entry.as_object_mut() else {
        return Vec::new();
    };

    let entry_id = entry_obj
        .get("ID")
        .and_then(serde_json::Value::as_u64)
        .map(|id| id.to_string());

    let mut preprocessed = entry_obj.clone();
    preprocess(&mut preprocessed);

    let mut lines = Vec::new();
    for (key, val) in &preprocessed {
        if key == "ID" {
            continue;
        }
        let field_name = FieldName::parse(key);
        let Some(field_value) = FieldValue::parse(val) else {
            continue;
        };
        lines.extend(emit_metric(
            host,
            prefix,
            &field_name,
            &field_value,
            entry_id.as_deref(),
        ));
    }
    lines
}

fn emit_metric(
    host: &str,
    prefix: &str,
    name: &FieldName,
    value: &FieldValue,
    entry_id: Option<&str>,
) -> Vec<String> {
    // Fan fields should only have scalar RPM values. Skip unexpected arrays.
    if matches!(
        (name, value),
        (FieldName::Fan { .. }, FieldValue::IndexedArray(_))
    ) {
        return Vec::new();
    }

    let metric = match name {
        FieldName::Plain(name) | FieldName::Indexed { name, .. } => format!("{prefix}_{name}"),
        FieldName::Fan { .. } => format!("{prefix}_fan"),
    };

    match value {
        FieldValue::Number(val) => {
            vec![emit_one(host, &metric, name, entry_id, None, *val)]
        }
        FieldValue::IndexedArray(arr) => arr
            .iter()
            .enumerate()
            .map(|(index, val)| emit_one(host, &metric, name, entry_id, Some(index), *val))
            .collect(),
    }
}

fn emit_one(
    host: &str,
    metric: &str,
    name: &FieldName,
    entry_id: Option<&str>,
    array_index: Option<usize>,
    val: f64,
) -> String {
    match name {
        FieldName::Plain(_) => match (array_index, entry_id) {
            (Some(index), _) => {
                let idx = index.to_string();
                metrics::gauge(metric, &[("host", host), ("idx", &idx)], val)
            }
            (None, Some(id)) => metrics::gauge(metric, &[("host", host), ("idx", id)], val),
            (None, None) => metrics::gauge(metric, &[("host", host)], val),
        },
        FieldName::Indexed { index, .. } => {
            let board = index.to_string();
            match array_index {
                Some(arr_idx) => {
                    let idx = arr_idx.to_string();
                    metrics::gauge(
                        metric,
                        &[("host", host), ("hashboard", &board), ("idx", &idx)],
                        val,
                    )
                }
                None => metrics::gauge(metric, &[("host", host), ("hashboard", &board)], val),
            }
        }
        FieldName::Fan { index } => {
            let idx = index.to_string();
            metrics::gauge(metric, &[("host", host), ("idx", &idx)], val)
        }
    }
}

/// Send a command to a cgminer instance and return the parsed JSON response.
pub async fn command(host: &str, port: u16, cmd: &str) -> Result<Value> {
    command_with_param(host, port, cmd, None).await
}

/// Send a command with an optional parameter to a cgminer instance.
pub async fn command_with_param(
    host: &str,
    port: u16,
    cmd: &str,
    param: Option<&str>,
) -> Result<Value> {
    let addr = format!("{host}:{port}");

    let mut stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr))
        .await
        .map_err(|_| anyhow::anyhow!("connect timeout: {addr}"))??;

    let request = match param {
        Some(p) => format!("{{\"command\":\"{cmd}\",\"parameter\":\"{p}\"}}\n"),
        None => format!("{{\"command\":\"{cmd}\"}}\n"),
    };

    stream.write_all(request.as_bytes()).await?;
    stream.shutdown().await?;

    let mut buf = Vec::with_capacity(4096);
    tokio::time::timeout(READ_TIMEOUT, stream.read_to_end(&mut buf))
        .await
        .map_err(|_| anyhow::anyhow!("read timeout: {addr}"))??;

    // Cgminer terminates responses with a NUL byte.
    if buf.last() == Some(&0) {
        buf.pop();
    }

    let value: Value = serde_json::from_slice(&buf)?;
    Ok(value)
}

#[cfg(test)]
#[path = "tests/cgminer.rs"]
mod tests;
