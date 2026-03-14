use super::*;

#[test]
fn gauge_no_labels() {
    let line = gauge("up", &[], 1.0);
    assert!(line.starts_with("up 1 "));
    let parts: Vec<&str> = line.split(' ').collect();
    assert_eq!(parts.len(), 3);
    let ts: u128 = parts[2].parse().expect("BUG: timestamp is numeric");
    assert!(ts > 1_700_000_000_000);
}

#[test]
fn gauge_with_labels() {
    let line = gauge(
        "pcb_temperature_celsius",
        &[("host", "10.0.0.1"), ("hashboard", "1")],
        65.0,
    );
    assert!(line.starts_with("pcb_temperature_celsius{host=\"10.0.0.1\",hashboard=\"1\"} 65 "));
}

#[test]
fn gauge_fractional_value() {
    let line = gauge(
        "hashboard_nominal_hashrate_gigahashes_per_second",
        &[("host", "10.0.0.1")],
        90858.66,
    );
    assert!(line.starts_with(
        "hashboard_nominal_hashrate_gigahashes_per_second{host=\"10.0.0.1\"} 90858.66 "
    ));
}

#[test]
fn gauge_zero() {
    let line = gauge("fan_rpm_feedback", &[], 0.0);
    assert!(line.starts_with("fan_rpm_feedback 0 "));
}

#[test]
fn format_value_integer() {
    assert_eq!(format_value(42.0), "42");
    assert_eq!(format_value(0.0), "0");
    assert_eq!(format_value(-1.0), "-1");
}

#[test]
fn format_value_fractional() {
    assert_eq!(format_value(23.5), "23.5");
    assert_eq!(format_value(90858.66), "90858.66");
}
