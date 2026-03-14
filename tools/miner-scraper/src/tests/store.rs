use super::*;

#[tokio::test]
async fn update_and_render() {
    let store = MetricsStore::new();
    store.update("10.0.0.1", vec!["up 1".to_owned()]).await;
    store.update("10.0.0.2", vec!["temp 23.5".to_owned()]).await;

    let output = store.render().await;
    assert!(output.contains("up 1\n"));
    assert!(output.contains("temp 23.5\n"));
}

#[tokio::test]
async fn remove_host() {
    let store = MetricsStore::new();
    store.update("10.0.0.1", vec!["up 1".to_owned()]).await;
    store.remove("10.0.0.1").await;

    let output = store.render().await;
    assert!(output.is_empty());
}

#[tokio::test]
async fn hosts_list() {
    let store = MetricsStore::new();
    store.update("10.0.0.1", vec!["up 1".to_owned()]).await;
    store.update("10.0.0.2", vec!["up 1".to_owned()]).await;

    let mut hosts = store.hosts().await;
    hosts.sort();
    assert_eq!(hosts, vec!["10.0.0.1", "10.0.0.2"]);
}
