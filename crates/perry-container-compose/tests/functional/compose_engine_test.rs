use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::ComposeSpec;
use perry_container_compose::testing::mock_backend::MockBackend;
use std::sync::Arc;

#[tokio::test]
async fn test_up_creates_networks_and_volumes() {
    let mut spec = ComposeSpec::default();
    spec.networks = Some([( "frontend".into(), None )].into_iter().collect());
    spec.volumes = Some([( "db_data".into(), None )].into_iter().collect());

    let backend = Arc::new(MockBackend::new());
    let engine = ComposeEngine::new(spec, "test_proj".into(), backend.clone());

    let _ = engine.up(&[], true, false, false).await.unwrap();

    let calls = backend.calls.lock().unwrap();
    assert!(calls.contains(&"create_network:frontend".to_string()));
    assert!(calls.contains(&"create_volume:db_data".to_string()));
}
