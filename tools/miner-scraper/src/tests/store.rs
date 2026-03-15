use super::*;
use crate::metrics::MetricBuilder;

fn test_metric(name: &str, value: f64) -> Metric {
    MetricBuilder::default()
        .name(name)
        .value(value)
        .build()
        .expect("BUG: name and value are set")
}

#[tokio::test]
async fn update_and_render() {
    let store = Store::new();
    store.update("10.0.0.1", vec![test_metric("up", 1.0)]).await;
    store
        .update("10.0.0.2", vec![test_metric("temp", 23.5)])
        .await;

    let output = store.render().await;
    assert!(output.contains("up 1 "));
    assert!(output.contains("temp 23.5 "));
}

#[tokio::test]
async fn remove_host() {
    let store = Store::new();
    store.update("10.0.0.1", vec![test_metric("up", 1.0)]).await;
    store.remove("10.0.0.1").await;

    let output = store.render().await;
    assert!(output.is_empty());
}

#[tokio::test]
async fn hosts_list() {
    let store = Store::new();
    store.update("10.0.0.1", vec![test_metric("up", 1.0)]).await;
    store.update("10.0.0.2", vec![test_metric("up", 1.0)]).await;

    let mut hosts = store.hosts().await;
    hosts.sort();
    assert_eq!(hosts, vec!["10.0.0.1", "10.0.0.2"]);
}
