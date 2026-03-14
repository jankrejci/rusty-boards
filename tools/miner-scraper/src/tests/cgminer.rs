use super::*;

fn is_error(response: &Value) -> bool {
    response
        .get("STATUS")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("STATUS"))
        .and_then(|s| s.as_str())
        == Some("E")
}

fn parse_raw(data: &[u8]) -> Result<Value> {
    let trimmed = if data.last() == Some(&0) {
        &data[..data.len() - 1]
    } else {
        data
    };
    let value: Value = serde_json::from_slice(trimmed)?;
    Ok(value)
}

const STOCK_STATS: &str = include_str!("../../dumps/stock-cgminer-stats-s21xp.json");
const STOCK_SUMMARY: &str = include_str!("../../dumps/stock-cgminer-summary-s21xp.json");
const STOCK_VERSION: &str = include_str!("../../dumps/stock-cgminer-version-s21xp.json");
const STOCK_DEVDETAILS: &str = include_str!("../../dumps/stock-cgminer-devdetails-s21xp.json");
const BRAIINS_TEMPS: &str = include_str!("../../dumps/braiins-cgminer-temps-s21plus.json");
const BRAIINS_FANS: &str = include_str!("../../dumps/braiins-cgminer-fans-s21plus.json");
const BRAIINS_SUMMARY: &str = include_str!("../../dumps/braiins-cgminer-summary-s21plus.json");
const LUXOS_STATS: &str = include_str!("../../dumps/luxos-cgminer-stats-s21pro.json");
const LUXOS_TEMPS: &str = include_str!("../../dumps/luxos-cgminer-temps-s21pro.json");
const LUXOS_FANS: &str = include_str!("../../dumps/luxos-cgminer-fans-s21pro.json");
const MARA_STATS: &str = include_str!("../../dumps/mara-cgminer-stats-s21imm.json");
const VNISH_STATS: &str = include_str!("../../dumps/vnish-cgminer-stats-s21.json");
const BRAIINS_DEVS: &str = include_str!("../../dumps/braiins-cgminer-devs-s21plus.json");
const BRAIINS_DEVDETAILS: &str =
    include_str!("../../dumps/braiins-cgminer-devdetails-s21plus.json");
const LUXOS_POWER: &str = include_str!("../../dumps/luxos-cgminer-power-s21pro.json");
const MARA_SUMMARY: &str = include_str!("../../dumps/mara-cgminer-summary-s21imm.json");
const VNISH_SUMMARY: &str = include_str!("../../dumps/vnish-cgminer-summary-s21.json");

/// Find all metric lines matching an exact metric name.
fn find_metric<'a>(lines: &'a [String], name: &str) -> Vec<&'a String> {
    lines
        .iter()
        .filter(|l| l.starts_with(name) && l.as_bytes().get(name.len()) == Some(&b'{'))
        .collect()
}

fn parse_json(data: &str) -> Value {
    serde_json::from_str(data).expect("BUG: dump data is valid JSON")
}

fn parse_to_lines(host: &str, data: &str) -> Vec<String> {
    let mut value = parse_json(data);
    parse_response(host, &mut value)
}

// --- FieldName tests ---

#[test]
fn field_name_plain() {
    assert_eq!(
        FieldName::parse("ghs_5s"),
        FieldName::Plain("ghs_5s".into())
    );
    assert_eq!(
        FieldName::parse("rate_30m"),
        FieldName::Plain("rate_30m".into())
    );
    assert_eq!(
        FieldName::parse("no_matching_work"),
        FieldName::Plain("no_matching_work".into())
    );
}

#[test]
fn field_name_indexed() {
    assert_eq!(
        FieldName::parse("temp_pcb1"),
        FieldName::Indexed {
            name: "temp_pcb".into(),
            index: 1
        }
    );
    assert_eq!(
        FieldName::parse("chain_rate3"),
        FieldName::Indexed {
            name: "chain_rate".into(),
            index: 3
        }
    );
    assert_eq!(
        FieldName::parse("freq1"),
        FieldName::Indexed {
            name: "freq".into(),
            index: 1
        }
    );
}

#[test]
fn field_name_indexed_with_underscore() {
    // temp2_1 → strip trailing "1", prefix "temp2_", strip underscores → "temp2".
    assert_eq!(
        FieldName::parse("temp2_1"),
        FieldName::Indexed {
            name: "temp2".into(),
            index: 1
        }
    );
}

#[test]
fn field_name_fan() {
    assert_eq!(FieldName::parse("fan1"), FieldName::Fan { index: 1 });
    assert_eq!(FieldName::parse("fan4"), FieldName::Fan { index: 4 });
}

#[test]
fn field_name_spaces_and_dots() {
    // "GHS 5s" → lowercase → "ghs 5s" → replace space → "ghs_5s" → ends in 's' → Plain.
    assert_eq!(
        FieldName::parse("GHS 5s"),
        FieldName::Plain("ghs_5s".into())
    );
}

#[test]
fn field_name_strips_percent() {
    assert_eq!(
        FieldName::parse("Device Hardware%"),
        FieldName::Plain("device_hardware".into())
    );
    assert_eq!(
        FieldName::parse("Pool Rejected%"),
        FieldName::Plain("pool_rejected".into())
    );
}

// --- FieldValue tests ---

#[test]
fn field_value_integer() {
    let v = serde_json::json!(42);
    assert_eq!(FieldValue::parse(&v), Some(FieldValue::Number(42.0)));
}

#[test]
fn field_value_float() {
    let v = serde_json::json!(90858.66);
    assert_eq!(FieldValue::parse(&v), Some(FieldValue::Number(90858.66)));
}

#[test]
fn field_value_string_number() {
    let v = serde_json::json!("90858.66");
    assert_eq!(FieldValue::parse(&v), Some(FieldValue::Number(90858.66)));
}

#[test]
fn field_value_array_of_numbers() {
    let v = serde_json::json!([43, 58, 65, 63]);
    assert_eq!(
        FieldValue::parse(&v),
        Some(FieldValue::IndexedArray(vec![43.0, 58.0, 65.0, 63.0]))
    );
}

#[test]
fn field_value_non_parseable_string() {
    let v = serde_json::json!("BTM_SOC0");
    assert_eq!(FieldValue::parse(&v), None);
}

#[test]
fn field_value_empty_string() {
    let v = serde_json::json!("");
    assert_eq!(FieldValue::parse(&v), None);
}

#[test]
fn field_value_boolean() {
    let v = serde_json::json!(false);
    assert_eq!(FieldValue::parse(&v), None);
}

#[test]
fn field_value_empty_array() {
    let v = serde_json::json!([]);
    assert_eq!(FieldValue::parse(&v), None);
}

// --- Preprocessing tests ---

#[test]
fn preprocess_dash_string_to_array() {
    let mut obj = serde_json::Map::new();
    obj.insert("temp_pcb1".into(), serde_json::json!("43-58-65-63"));
    preprocess(&mut obj);
    assert_eq!(
        obj["temp_pcb1"],
        serde_json::json!([43.0, 58.0, 65.0, 63.0])
    );
}

#[test]
fn preprocess_leaves_non_dash_strings() {
    let mut obj = serde_json::Map::new();
    obj.insert("miner_id".into(), serde_json::json!("no miner id now"));
    obj.insert("chain_rate1".into(), serde_json::json!("90858.66"));
    preprocess(&mut obj);
    assert_eq!(obj["miner_id"], serde_json::json!("no miner id now"));
    assert_eq!(obj["chain_rate1"], serde_json::json!("90858.66"));
}

#[test]
fn preprocess_leaves_numbers() {
    let mut obj = serde_json::Map::new();
    obj.insert("fan1".into(), serde_json::json!(3540));
    preprocess(&mut obj);
    assert_eq!(obj["fan1"], serde_json::json!(3540));
}

#[test]
fn preprocess_all_zeros() {
    let mut obj = serde_json::Map::new();
    obj.insert("temp_pcb4".into(), serde_json::json!("0-0-0-0"));
    preprocess(&mut obj);
    assert_eq!(obj["temp_pcb4"], serde_json::json!([0.0, 0.0, 0.0, 0.0]));
}

// --- Raw parsing tests ---

#[test]
fn raw_parse_stock_stats() {
    let data = STOCK_STATS.as_bytes();
    let value = parse_raw(data).expect("BUG: sample data is valid JSON");
    assert!(value.get("STATS").is_some());
    let stats = value["STATS"].as_array().expect("BUG: STATS is array");
    assert_eq!(stats.len(), 2);
}

#[test]
fn raw_parse_stock_summary() {
    let data = STOCK_SUMMARY.as_bytes();
    let value = parse_raw(data).expect("BUG: sample data is valid JSON");
    let summary = &value["SUMMARY"][0];
    assert!(summary["GHS 5s"].as_f64().is_some());
}

#[test]
fn raw_parse_stock_version() {
    let data = STOCK_VERSION.as_bytes();
    let value = parse_raw(data).expect("BUG: sample data is valid JSON");
    let version = &value["VERSION"][0];
    assert_eq!(version["Type"].as_str(), Some("Antminer S21 XP"));
}

#[test]
fn devdetails_is_error() {
    let data = STOCK_DEVDETAILS.as_bytes();
    let value = parse_raw(data).expect("BUG: sample data is valid JSON");
    assert!(is_error(&value));
}

#[test]
fn stats_is_not_error() {
    let data = STOCK_STATS.as_bytes();
    let value = parse_raw(data).expect("BUG: sample data is valid JSON");
    assert!(!is_error(&value));
}

// --- Integration tests: stock ---

#[test]
fn stock_stats_emits_fans() {
    let lines = parse_to_lines("10.0.0.1", STOCK_STATS);
    let fans = find_metric(&lines, "stats_fan");
    assert_eq!(fans.len(), 4);
    assert!(fans[0].contains("idx=\"1\""));
}

#[test]
fn stock_stats_emits_temperatures() {
    let lines = parse_to_lines("10.0.0.1", STOCK_STATS);
    let pcb = find_metric(&lines, "stats_temp_pcb");
    assert_eq!(pcb.len(), 16);
    assert!(pcb[0].contains("hashboard=\"1\""));
}

#[test]
fn stock_summary_emits_hashrate() {
    let lines = parse_to_lines("10.0.0.1", STOCK_SUMMARY);
    let ghs = find_metric(&lines, "summary_ghs_5s");
    assert_eq!(ghs.len(), 1);
}

// --- Integration tests: braiins ---

#[test]
fn braiins_temps_emits_metrics_with_idx() {
    let lines = parse_to_lines("10.0.0.1", BRAIINS_TEMPS);
    let board = find_metric(&lines, "temps_board");
    assert_eq!(board.len(), 3);
    assert!(board[0].contains("idx=\"1\""));
    assert!(board[2].contains("idx=\"3\""));
    let chip = find_metric(&lines, "temps_chip");
    assert_eq!(chip.len(), 3);
    assert!(chip[0].contains("idx=\"1\""));
}

#[test]
fn braiins_fans_emits_rpm_with_idx() {
    let lines = parse_to_lines("10.0.0.1", BRAIINS_FANS);
    let rpm = find_metric(&lines, "fans_rpm");
    assert_eq!(rpm.len(), 4);
    assert!(rpm[0].contains("idx=\"0\""));
    assert!(rpm[3].contains("idx=\"3\""));
}

#[test]
fn braiins_summary_emits_hashrate() {
    let lines = parse_to_lines("10.0.0.1", BRAIINS_SUMMARY);
    let mhs = find_metric(&lines, "summary_mhs_5s");
    assert_eq!(mhs.len(), 1);
}

// --- Integration tests: luxos ---

#[test]
fn luxos_stats_emits_chain_rates() {
    let lines = parse_to_lines("10.0.0.1", LUXOS_STATS);
    let rates = find_metric(&lines, "stats_chain_rate");
    // 3 active boards + chain_rate4="0" (parseable as f64).
    assert_eq!(rates.len(), 4);
}

#[test]
fn luxos_temps_emits_board_temps() {
    let lines = parse_to_lines("10.0.0.1", LUXOS_TEMPS);
    let bottom_left = find_metric(&lines, "temps_boardbottomleft");
    assert!(!bottom_left.is_empty());
}

#[test]
fn luxos_fans_emits_rpm_with_idx() {
    let lines = parse_to_lines("10.0.0.1", LUXOS_FANS);
    let rpm = find_metric(&lines, "fans_rpm");
    assert_eq!(rpm.len(), 4);
    assert!(rpm[0].contains("idx=\"0\""));
    assert!(rpm[3].contains("idx=\"3\""));
    // ID should not be emitted as its own metric.
    let id = find_metric(&lines, "fans_id");
    assert!(id.is_empty());
}

// --- Integration tests: mara ---

#[test]
fn mara_stats_emits_chain_rates() {
    let lines = parse_to_lines("10.0.0.1", MARA_STATS);
    let rates = find_metric(&lines, "stats_chain_rate");
    assert_eq!(rates.len(), 3);
}

#[test]
fn mara_stats_emits_temperatures() {
    let lines = parse_to_lines("10.0.0.1", MARA_STATS);
    let pcb = find_metric(&lines, "stats_temp_pcb");
    // Board 1 and 2 have 2 sensors each, boards 3 and 4 have 4 zeros each.
    assert!(!pcb.is_empty());
}

// --- Integration tests: vnish ---

#[test]
fn vnish_stats_emits_chain_rates() {
    let lines = parse_to_lines("10.0.0.1", VNISH_STATS);
    let rates = find_metric(&lines, "stats_chain_rate");
    assert_eq!(rates.len(), 3);
}

#[test]
fn vnish_stats_emits_temperatures() {
    let lines = parse_to_lines("10.0.0.1", VNISH_STATS);
    let pcb = find_metric(&lines, "stats_temp_pcb");
    assert!(!pcb.is_empty());
    let chip = find_metric(&lines, "stats_temp_chip");
    assert!(!chip.is_empty());
}

// --- Integration tests: stock (detailed) ---

#[test]
fn stock_stats_hashrates_per_board() {
    let lines = parse_to_lines("10.0.0.1", STOCK_STATS);
    let rates = find_metric(&lines, "stats_chain_rate");
    // 3 boards with rates, chain_rate4 is empty string.
    assert_eq!(rates.len(), 3);
    assert!(rates[0].contains("hashboard=\"1\""));
    assert!(rates[0].contains("90858.66"));
}

#[test]
fn stock_stats_frequencies_per_board() {
    let lines = parse_to_lines("10.0.0.1", STOCK_STATS);
    let freqs = find_metric(&lines, "stats_freq");
    // 4 boards including freq4=0.
    assert_eq!(freqs.len(), 4);
    assert!(freqs[0].contains("hashboard=\"1\""));
    assert!(freqs[0].contains(" 490 "));
}

#[test]
fn stock_stats_chain_acn_per_board() {
    let lines = parse_to_lines("10.0.0.1", STOCK_STATS);
    let acn = find_metric(&lines, "stats_chain_acn");
    assert_eq!(acn.len(), 4);
    assert!(acn[0].contains("hashboard=\"1\""));
    assert!(acn[0].contains(" 91 "));
    assert!(acn[3].contains("hashboard=\"4\""));
    assert!(acn[3].contains(" 0 "));
}

#[test]
fn stock_stats_miner_level_metrics() {
    let lines = parse_to_lines("10.0.0.1", STOCK_STATS);
    assert!(find_metric(&lines, "stats_ghs_5s")[0].contains("275629.88"));
    assert!(find_metric(&lines, "stats_ghs_av")[0].contains("272136.87"));
    assert!(find_metric(&lines, "stats_rate_30m")[0].contains("267931.05"));
    assert!(find_metric(&lines, "stats_total_rateideal")[0].contains("270000"));
    assert!(find_metric(&lines, "stats_total_freqavg")[0].contains(" 490 "));
    assert!(find_metric(&lines, "stats_total_acn")[0].contains(" 273 "));
    assert!(find_metric(&lines, "stats_total_rate")[0].contains("272136.87"));
    assert_eq!(find_metric(&lines, "stats_no_matching_work").len(), 1);
}

#[test]
fn stock_stats_host_label_present() {
    let lines = parse_to_lines("10.36.1.51", STOCK_STATS);
    for line in &lines {
        assert!(
            line.contains("host=\"10.36.1.51\""),
            "missing host label in: {}",
            line
        );
    }
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

// --- Full dump parsing tests ---

#[test]
fn stock_all_dumps_produce_metrics() {
    let mut lines = Vec::new();
    for data in [STOCK_STATS, STOCK_SUMMARY] {
        let value = parse_json(data);
        lines.extend(parse_response("10.0.0.1", &value));
    }
    assert!(
        lines.len() > 50,
        "stock should produce >50 metrics, got {}",
        lines.len()
    );
}

#[test]
fn braiins_all_dumps_produce_metrics() {
    let mut lines = Vec::new();
    for data in [
        BRAIINS_TEMPS,
        BRAIINS_FANS,
        BRAIINS_SUMMARY,
        BRAIINS_DEVS,
        BRAIINS_DEVDETAILS,
    ] {
        let value = parse_json(data);
        if !is_error(&value) {
            lines.extend(parse_response("10.0.0.1", &value));
        }
    }
    assert!(
        lines.len() > 50,
        "braiins should produce >50 metrics, got {}",
        lines.len()
    );
}

#[test]
fn luxos_all_dumps_produce_metrics() {
    let mut lines = Vec::new();
    for data in [LUXOS_STATS, LUXOS_TEMPS, LUXOS_FANS, LUXOS_POWER] {
        let value = parse_json(data);
        if !is_error(&value) {
            lines.extend(parse_response("10.0.0.1", &value));
        }
    }
    assert!(
        lines.len() > 50,
        "luxos should produce >50 metrics, got {}",
        lines.len()
    );
}

#[test]
fn vnish_all_dumps_produce_metrics() {
    let mut lines = Vec::new();
    for data in [VNISH_STATS, VNISH_SUMMARY] {
        let value = parse_json(data);
        lines.extend(parse_response("10.0.0.1", &value));
    }
    assert!(
        lines.len() > 50,
        "vnish should produce >50 metrics, got {}",
        lines.len()
    );
}

#[test]
fn mara_all_dumps_produce_metrics() {
    let mut lines = Vec::new();
    for data in [MARA_STATS, MARA_SUMMARY] {
        let value = parse_json(data);
        lines.extend(parse_response("10.0.0.1", &value));
    }
    assert!(
        lines.len() > 50,
        "mara should produce >50 metrics, got {}",
        lines.len()
    );
}

// --- STATUS and id skipping ---

#[test]
fn skips_status_and_id() {
    let lines = parse_to_lines("10.0.0.1", STOCK_STATS);
    for line in &lines {
        assert!(
            !line.starts_with("status_"),
            "STATUS should be skipped: {}",
            line
        );
    }
    // The "id" field at the top level should not produce a metric.
    assert!(find_metric(&lines, "stats_id").is_empty());
}
