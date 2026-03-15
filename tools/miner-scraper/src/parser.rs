//! Generic JSON-to-metric parser for cgminer API responses.
//!
//! Converts arbitrary cgminer JSON into structured metric data by normalizing
//! field names, converting dash-separated strings to arrays, and extracting
//! labeled numeric values.

use serde_json::Value;

#[cfg(test)]
#[path = "tests/parser.rs"]
mod tests;

/// Structured metric extracted from a cgminer JSON response.
pub struct ParsedLine {
    pub name: String,
    pub labels: Vec<(String, String)>,
    pub value: f64,
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
            Value::String(s) => s.parse::<f64>().ok().map(FieldValue::Number),
            Value::Array(arr) => parse_numeric_array(arr),
            _ => None,
        }
    }
}

/// Parse a JSON array into an indexed array if all elements are numbers.
fn parse_numeric_array(arr: &[Value]) -> Option<FieldValue> {
    if arr.is_empty() {
        return None;
    }
    let nums: Vec<f64> = arr.iter().filter_map(serde_json::Value::as_f64).collect();
    if nums.len() != arr.len() {
        return None;
    }
    Some(FieldValue::IndexedArray(nums))
}

/// Convert dash-separated numeric strings to JSON arrays in place.
///
/// Cgminer firmware encodes per-chip sensor readings as dash-separated strings
/// like `"43-58-65-63"`. This function detects that pattern and replaces the
/// string value with a JSON array `[43, 58, 65, 63]` so the metric parser can
/// extract indexed values. Non-numeric or single-element strings are left
/// unchanged.
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

/// Parse a cgminer JSON response into structured metrics.
///
/// Iterates all data sections in the response (skipping STATUS and id),
/// preprocesses dash-separated strings into arrays, then extracts each numeric
/// field as a `ParsedLine`. The section name is lowercased and prepended to
/// each metric name (e.g. FANS.RPM becomes `fans_rpm`).
pub fn parse_response(response: &mut Value) -> Vec<ParsedLine> {
    let Some(obj) = response.as_object_mut() else {
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
            metrics.extend(parse_entry(&prefix, entry));
        }
    }
    metrics
}

fn parse_entry(prefix: &str, entry: &mut Value) -> Vec<ParsedLine> {
    let Some(entry_obj) = entry.as_object_mut() else {
        return Vec::new();
    };

    let entry_id = entry_obj
        .get("ID")
        .and_then(serde_json::Value::as_u64)
        .map(|id| id.to_string());

    preprocess(entry_obj);

    let mut metrics = Vec::new();
    for (key, val) in entry_obj.iter() {
        if key == "ID" {
            continue;
        }
        let field_name = FieldName::parse(key);
        let Some(field_value) = FieldValue::parse(val) else {
            continue;
        };
        metrics.extend(emit_metric(
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
            vec![emit_one(&metric, name, entry_id, None, *val)]
        }
        FieldValue::IndexedArray(arr) => arr
            .iter()
            .enumerate()
            .map(|(index, val)| emit_one(&metric, name, entry_id, Some(index), *val))
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
