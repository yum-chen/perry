use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::testing::mock_backend::MockBackend;
use perry_container_compose::types::ComposeSpec;
use std::sync::Arc;

#[tokio::test]
async fn test_compose_up_simple() {
    let yaml = r#"
services:
  web:
    image: nginx
"#;
    let spec = ComposeSpec::parse_str(yaml).unwrap();
    let backend = Arc::new(MockBackend::new("mock"));
    let engine = Arc::new(ComposeEngine::new(spec, "test".to_string(), backend.clone()));

    let _handle = engine.up(&[], false, false, false).await.unwrap();

    let calls = backend.calls.lock().unwrap();
    assert!(calls.iter().any(|c| matches!(c, perry_container_compose::testing::mock_backend::RecordedCall::Run(_))));
}
