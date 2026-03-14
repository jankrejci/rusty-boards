use super::*;

#[test]
fn parse_minimal_config() {
    let toml = r#"
targets = ["10.36.1.51"]
"#;
    let config: Config = toml::from_str(toml).expect("BUG: test toml is valid");
    assert_eq!(config.listen, "127.0.0.1:8889");
    assert_eq!(config.scrape_interval_secs, 5);
    assert_eq!(config.targets, vec!["10.36.1.51"]);
}

#[test]
fn parse_full_config() {
    let toml = r#"
listen = "127.0.0.1:9090"
scrape_interval_secs = 30
targets = ["10.36.1.51", "10.36.1.52"]
"#;
    let config: Config = toml::from_str(toml).expect("BUG: test toml is valid");
    assert_eq!(config.listen, "127.0.0.1:9090");
    assert_eq!(config.scrape_interval_secs, 30);
    assert_eq!(config.targets.len(), 2);
}

#[test]
fn empty_targets_defaults() {
    let toml = r#"
listen = "0.0.0.0:8081"
"#;
    let config: Config = toml::from_str(toml).expect("BUG: test toml is valid");
    assert!(config.targets.is_empty());
}
