use super::*;
use crate::metrics::MetricBuilder;
use tokio::sync::mpsc;

fn test_metric(name: &str, value: f64) -> Metric {
    MetricBuilder::default()
        .name(name)
        .value(value)
        .build()
        .expect("BUG: name and value are set")
}

/// Create a store and return its handle for testing.
///
/// The store itself is not started since tests interact through the handle.
fn test_handle() -> StoreState {
    let (_tx, rx) = mpsc::channel(1);
    Store::new(rx).state()
}

#[tokio::test]
async fn update_and_render() {
    let handle = test_handle();
    handle
        .update("10.0.0.1", vec![test_metric("up", 1.0)])
        .await;
    handle
        .update("10.0.0.2", vec![test_metric("temp", 23.5)])
        .await;

    let output = handle.render().await;
    assert!(output.contains("up 1 "));
    assert!(output.contains("temp 23.5 "));
}

#[tokio::test]
async fn remove_host() {
    let handle = test_handle();
    handle
        .update("10.0.0.1", vec![test_metric("up", 1.0)])
        .await;
    handle.remove("10.0.0.1").await;

    let output = handle.render().await;
    assert!(output.is_empty());
}

#[tokio::test]
async fn hosts_list() {
    let handle = test_handle();
    handle
        .update("10.0.0.1", vec![test_metric("up", 1.0)])
        .await;
    handle
        .update("10.0.0.2", vec![test_metric("up", 1.0)])
        .await;

    let mut hosts = handle.hosts().await;
    hosts.sort();
    assert_eq!(hosts, vec!["10.0.0.1", "10.0.0.2"]);
}
