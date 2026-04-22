use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::testing::mock_backend::MockBackend;
use perry_container_compose::types::{ComposeSpec, ComposeService};
use std::sync::Arc;
use indexmap::IndexMap;

#[tokio::test]
async fn up_creates_networks_before_containers() {
    let mock = MockBackend::new();
    let mut services = IndexMap::new();
    services.insert("web".to_string(), ComposeService {
        image: Some("nginx".to_string()),
        ..Default::default()
    });

    let mut networks = IndexMap::new();
    networks.insert("frontend".to_string(), None);

    let spec = ComposeSpec {
        services,
        networks: Some(networks),
        ..Default::default()
    };

    let engine = ComposeEngine::new(spec, "test".into(), Arc::new(mock));
    engine.up(&[], true, false, false).await.unwrap();

    let calls = engine.backend.backend_name(); // Wait, how to access mock from engine.backend (Arc<dyn ContainerBackend>)?
    // The engine's backend is public in my implementation.
}
