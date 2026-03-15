use super::*;

#[test]
fn parse_minimal_config() {
    let toml = r#"
targets = ["10.36.1.51"]
"#;
    let config: Config = toml::from_str(toml).expect("BUG: test toml is valid");
    assert_eq!(config.listen, "127.0.0.1:8889");
    assert_eq!(config.targets, vec!["10.36.1.51"]);
    assert_eq!(config.tiers.high_secs, 1);
    assert_eq!(config.tiers.mid_secs, 10);
    assert_eq!(config.tiers.low_secs, 60);
}

#[test]
fn parse_full_config() {
    let toml = r#"
listen = "127.0.0.1:9090"
targets = ["10.36.1.51", "10.36.1.52"]

[tiers]
high_secs = 2
mid_secs = 15
low_secs = 120
"#;
    let config: Config = toml::from_str(toml).expect("BUG: test toml is valid");
    assert_eq!(config.listen, "127.0.0.1:9090");
    assert_eq!(config.targets.len(), 2);
    assert_eq!(config.tiers.high_secs, 2);
    assert_eq!(config.tiers.mid_secs, 15);
    assert_eq!(config.tiers.low_secs, 120);
}

#[test]
fn empty_targets_defaults() {
    let toml = r#"
listen = "0.0.0.0:8081"
"#;
    let config: Config = toml::from_str(toml).expect("BUG: test toml is valid");
    assert!(config.targets.is_empty());
}
