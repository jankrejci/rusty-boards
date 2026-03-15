use super::*;

#[test]
fn display_no_labels() {
    let metric = MetricBuilder::default().name("up").value(1.0).build();
    let metric = metric.expect("BUG: name and value are set");
    let line = metric.to_string();
    assert!(line.starts_with("up 1 "));
    let parts: Vec<&str> = line.split(' ').collect();
    assert_eq!(parts.len(), 3);
    let ts: u128 = parts[2].parse().expect("BUG: timestamp is numeric");
    assert!(ts > 1_700_000_000_000);
}

#[test]
fn display_with_labels() {
    let metric = MetricBuilder::default()
        .name("pcb_temperature_celsius")
        .label("hashboard", "1")
        .label("host", "10.0.0.1")
        .value(65.0)
        .build()
        .expect("BUG: name and value are set");
    let line = metric.to_string();
    // Labels are sorted alphabetically.
    assert!(line.starts_with("pcb_temperature_celsius{hashboard=\"1\",host=\"10.0.0.1\"} 65 "));
}

#[test]
fn display_fractional_value() {
    let metric = MetricBuilder::default()
        .name("hashboard_nominal_hashrate_gigahashes_per_second")
        .label("host", "10.0.0.1")
        .value(90858.66)
        .build()
        .expect("BUG: name and value are set");
    let line = metric.to_string();
    assert!(line.starts_with(
        "hashboard_nominal_hashrate_gigahashes_per_second{host=\"10.0.0.1\"} 90858.66 "
    ));
}

#[test]
fn display_zero() {
    let metric = MetricBuilder::default()
        .name("fan_rpm_feedback")
        .value(0.0)
        .build()
        .expect("BUG: name and value are set");
    assert!(metric.to_string().starts_with("fan_rpm_feedback 0 "));
}

#[test]
fn display_integer_formatting() {
    let metric = MetricBuilder::default()
        .name("test")
        .value(42.0)
        .build()
        .expect("BUG: name and value are set");
    assert!(metric.to_string().starts_with("test 42 "));

    let metric = MetricBuilder::default()
        .name("test")
        .value(-1.0)
        .build()
        .expect("BUG: name and value are set");
    assert!(metric.to_string().starts_with("test -1 "));
}

#[test]
fn display_fractional_formatting() {
    let metric = MetricBuilder::default()
        .name("test")
        .value(23.5)
        .build()
        .expect("BUG: name and value are set");
    assert!(metric.to_string().starts_with("test 23.5 "));
}

#[test]
fn builder_missing_name() {
    let result = MetricBuilder::default().value(1.0).build();
    assert!(result.is_none());
}

#[test]
fn builder_missing_value() {
    let result = MetricBuilder::default().name("up").build();
    assert!(result.is_none());
}

#[test]
fn builder_extend_labels() {
    let metric = MetricBuilder::default()
        .name("test")
        .label("host", "10.0.0.1")
        .extend_labels(vec![
            ("hashboard".to_string(), "1".to_string()),
            ("idx".to_string(), "0".to_string()),
        ])
        .value(42.0)
        .build()
        .expect("BUG: name and value are set");
    let line = metric.to_string();
    assert!(line.contains("host=\"10.0.0.1\""));
    assert!(line.contains("hashboard=\"1\""));
    assert!(line.contains("idx=\"0\""));
}
