use super::*;

const STOCK_STATS: &str = include_str!("../../dumps/stock-cgminer-stats-s21xp.json");
const STOCK_DEVDETAILS: &str = include_str!("../../dumps/stock-cgminer-devdetails-s21xp.json");
const BRAIINS_SUMMARY: &str = include_str!("../../dumps/braiins-cgminer-summary-s21plus.json");
const LUXOS_STATS: &str = include_str!("../../dumps/luxos-cgminer-stats-s21pro.json");
const MARA_STATS: &str = include_str!("../../dumps/mara-cgminer-stats-s21imm.json");
const VNISH_STATS: &str = include_str!("../../dumps/vnish-cgminer-stats-s21.json");

fn parse_json(data: &str) -> serde_json::Value {
    serde_json::from_str(data).expect("BUG: dump data is valid JSON")
}

// --- Firmware detection tests ---

#[test]
fn detect_stock_firmware() {
    let stats = parse_json(STOCK_STATS);
    assert_eq!(Firmware::identify(&stats), Firmware::Stock);
}

#[test]
fn detect_braiins_firmware() {
    let resp = parse_json(BRAIINS_SUMMARY);
    assert_eq!(Firmware::identify(&resp), Firmware::Braiins);
}

#[test]
fn detect_luxos_firmware() {
    let stats = parse_json(LUXOS_STATS);
    assert_eq!(Firmware::identify(&stats), Firmware::LuxOS);
}

#[test]
fn detect_mara_firmware() {
    let stats = parse_json(MARA_STATS);
    assert_eq!(Firmware::identify(&stats), Firmware::Mara);
}

#[test]
fn detect_vnish_firmware() {
    let stats = parse_json(VNISH_STATS);
    assert_eq!(Firmware::identify(&stats), Firmware::Vnish);
}

#[test]
fn detect_fallback_to_stock() {
    let empty = serde_json::json!({
        "STATUS": [{
            "Description": ""
        }]
    });
    assert_eq!(Firmware::identify(&empty), Firmware::Stock);
}

// --- is_error tests ---

#[test]
fn is_error_returns_true_for_error_response() {
    let resp = serde_json::json!({
        "STATUS": [{"STATUS": "E", "Msg": "Invalid command"}]
    });
    assert!(is_error(&resp));
}

#[test]
fn is_error_returns_false_for_success() {
    let resp = serde_json::json!({
        "STATUS": [{"STATUS": "S", "Msg": "OK"}]
    });
    assert!(!is_error(&resp));
}

#[test]
fn is_error_returns_false_for_empty_status() {
    let resp = serde_json::json!({});
    assert!(!is_error(&resp));
}

#[test]
fn devdetails_is_error() {
    let value = parse_json(STOCK_DEVDETAILS);
    assert!(is_error(&value));
}

#[test]
fn stats_is_not_error() {
    let value = parse_json(STOCK_STATS);
    assert!(!is_error(&value));
}

// --- Endpoint and tier tests ---

#[test]
fn stats_is_high_tier() {
    assert_eq!(
        Endpoint::Cgminer("stats", ScrapeTier::High).tier(),
        ScrapeTier::High
    );
}

#[test]
fn summary_is_mid_tier() {
    assert_eq!(
        Endpoint::Cgminer("summary", ScrapeTier::Mid).tier(),
        ScrapeTier::Mid
    );
}

#[test]
fn version_is_low_tier() {
    assert_eq!(
        Endpoint::Cgminer("version", ScrapeTier::Low).tier(),
        ScrapeTier::Low
    );
}

#[test]
fn endpoints_contains_known_commands() {
    assert!(ENDPOINTS.len() > 10);
    assert!(ENDPOINTS.iter().any(|e| e.command() == "stats"));
    assert!(ENDPOINTS.iter().any(|e| e.command() == "version"));
    assert!(ENDPOINTS.iter().any(|e| e.command() == "summary"));
}
