//! Generic JSON-to-metric parser for cgminer API responses.
//!
//! Converts arbitrary cgminer JSON into structured metric data by normalizing
//! field names, converting dash-separated strings to arrays, and extracting
//! labeled numeric values.

use std::sync::LazyLock;

use regex::Regex;

#[cfg(test)]
#[path = "tests/parser.rs"]
mod tests;

/// Structured metric extracted from a miner response.
pub struct ParsedLine {
    pub name: String,
    pub labels: Vec<(String, String)>,
    pub value: f64,
}

/// Common interface for response parsers.
pub trait Parser {
    fn parse(&mut self) -> Vec<ParsedLine>;
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
    /// underscores, then strips a trailing digit as the board or fan index.
    pub fn parse(key: &str) -> Self {
        let normalized: String = key
            .to_lowercase()
            .replace(['.', ' '], "_")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let normalized = normalized.trim_end_matches('_').to_string();

        let bytes = normalized.as_bytes();
        let Some(&last) = bytes.last() else {
            return FieldName::Plain(normalized);
        };

        if !last.is_ascii_digit() {
            return FieldName::Plain(normalized);
        }

        let index = u32::from(last - b'0');
        let name = normalized[..normalized.len() - 1].trim_end_matches('_');

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
    pub fn parse(value: &serde_json::Value) -> Option<Self> {
        match value {
            serde_json::Value::Number(n) => n.as_f64().map(FieldValue::Number),
            serde_json::Value::String(s) => s.parse::<f64>().ok().map(FieldValue::Number),
            serde_json::Value::Array(arr) => Self::parse_numeric_array(arr),
            _ => None,
        }
    }

    /// Parse a JSON array into an indexed array if all elements are numbers.
    fn parse_numeric_array(arr: &[serde_json::Value]) -> Option<Self> {
        if arr.is_empty() {
            return None;
        }
        let nums: Vec<f64> = arr.iter().filter_map(serde_json::Value::as_f64).collect();
        if nums.len() != arr.len() {
            return None;
        }
        Some(FieldValue::IndexedArray(nums))
    }
}

// --- JsonParser ---

/// JSON response parser for cgminer API responses.
///
/// Takes ownership of a JSON response, preprocesses dash-separated strings
/// into arrays, and extracts labeled numeric values as `ParsedLine` structs.
pub struct JsonParser {
    response: serde_json::Value,
}

impl JsonParser {
    pub fn new(response: serde_json::Value) -> Self {
        Self { response }
    }

    fn parse_entry(prefix: &str, entry: &mut serde_json::Value) -> Vec<ParsedLine> {
        let Some(entry_obj) = entry.as_object_mut() else {
            return Vec::new();
        };

        let entry_id = entry_obj
            .get("ID")
            .and_then(serde_json::Value::as_u64)
            .map(|id| id.to_string());

        Self::preprocess(entry_obj);

        let mut metrics = Vec::new();
        for (key, val) in entry_obj.iter() {
            if key == "ID" {
                continue;
            }
            let field_name = FieldName::parse(key);
            let Some(field_value) = FieldValue::parse(val) else {
                continue;
            };
            metrics.extend(Self::emit_metric(
                prefix,
                &field_name,
                &field_value,
                entry_id.as_deref(),
            ));
        }
        metrics
    }

    fn emit_metric(
        prefix: &str,
        name: &FieldName,
        value: &FieldValue,
        entry_id: Option<&str>,
    ) -> Vec<ParsedLine> {
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
                vec![Self::emit_one(&metric, name, entry_id, None, *val)]
            }
            FieldValue::IndexedArray(arr) => arr
                .iter()
                .enumerate()
                .map(|(index, val)| Self::emit_one(&metric, name, entry_id, Some(index), *val))
                .collect(),
        }
    }

    fn emit_one(
        metric: &str,
        name: &FieldName,
        entry_id: Option<&str>,
        array_index: Option<usize>,
        val: f64,
    ) -> ParsedLine {
        let mut labels = Vec::new();

        match name {
            FieldName::Plain(_) => match (array_index, entry_id) {
                (Some(index), _) => {
                    labels.push(("idx".to_string(), index.to_string()));
                }
                (None, Some(id)) => {
                    labels.push(("idx".to_string(), id.to_string()));
                }
                (None, None) => {}
            },
            FieldName::Indexed { index, .. } => {
                labels.push(("hashboard".to_string(), index.to_string()));
                if let Some(arr_idx) = array_index {
                    labels.push(("idx".to_string(), arr_idx.to_string()));
                }
            }
            FieldName::Fan { index } => {
                labels.push(("idx".to_string(), index.to_string()));
            }
        }

        ParsedLine {
            name: metric.to_string(),
            labels,
            value: val,
        }
    }

    /// Convert dash-separated numeric strings to JSON arrays in place.
    fn preprocess(obj: &mut serde_json::Map<String, serde_json::Value>) {
        for value in obj.values_mut() {
            let arr = match value {
                serde_json::Value::String(s) => Self::parse_dash_array(s),
                _ => continue,
            };
            let Some(arr) = arr else {
                continue;
            };
            *value = serde_json::Value::Array(arr);
        }
    }

    fn parse_dash_array(s: &str) -> Option<Vec<serde_json::Value>> {
        if !s.contains('-') {
            return None;
        }
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() < 2 {
            return None;
        }
        let nums: Vec<serde_json::Value> = parts
            .iter()
            .filter_map(|p| p.parse::<f64>().ok())
            .map(serde_json::Value::from)
            .collect();
        if nums.len() != parts.len() {
            return None;
        }
        Some(nums)
    }
}

impl Parser for JsonParser {
    /// Parse the response into structured metrics.
    ///
    /// Iterates all data sections in the response (skipping STATUS and id),
    /// preprocesses dash-separated strings into arrays, then extracts each numeric
    /// field as a `ParsedLine`.
    fn parse(&mut self) -> Vec<ParsedLine> {
        let Some(obj) = self.response.as_object_mut() else {
            return Vec::new();
        };

        let mut metrics = Vec::new();
        for (section, value) in obj {
            if section == "STATUS" || section == "id" {
                continue;
            }

            let prefix = section.to_lowercase();
            let Some(entries) = value.as_array_mut() else {
                continue;
            };

            for entry in entries {
                metrics.extend(Self::parse_entry(&prefix, entry));
            }
        }
        metrics
    }
}

// --- PlainParser ---

static READVOL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Regex cannot fail; the pattern is a compile-time constant.
    Regex::new(r"voltage:(?P<voltage>\d+)\s+feedback:(?P<feedback>[\d.]+)\s+power status:(?P<power_status>\d+)")
        .expect("BUG: readvol regex is valid")
});

static READVOL_PATTERNS: &[&LazyLock<Regex>] = &[&READVOL_PATTERN];

/// Return the regex patterns for a given endpoint command.
fn patterns_for(command: &str) -> &'static [&'static LazyLock<Regex>] {
    match command {
        "readvol" => READVOL_PATTERNS,
        _ => &[],
    }
}

/// Plain text response parser using regex-based extraction.
///
/// Each endpoint command maps to a set of compiled regex patterns with named
/// capture groups. Matched groups that parse as f64 become metrics with the
/// name `{prefix}_{capture_name}`.
pub struct PlainParser {
    text: String,
    prefix: String,
}

impl PlainParser {
    pub fn new(text: String, prefix: String) -> Self {
        Self { text, prefix }
    }
}

impl Parser for PlainParser {
    fn parse(&mut self) -> Vec<ParsedLine> {
        let patterns = patterns_for(&self.prefix);
        let mut metrics = Vec::new();
        for pattern in patterns {
            let Some(caps) = pattern.captures(&self.text) else {
                continue;
            };
            for name in pattern.capture_names().flatten() {
                let Some(value) = caps.name(name).and_then(|m| m.as_str().parse::<f64>().ok())
                else {
                    continue;
                };
                metrics.push(ParsedLine {
                    name: format!("{}_{}", self.prefix, name),
                    labels: Vec::new(),
                    value,
                });
            }
        }
        metrics
    }
}
