use perry_container_compose::compose::WorkloadGraphEngine;
use perry_container_compose::testing::mock_backend::MockBackend;
use std::sync::Arc;

#[tokio::test]
async fn test_workload_run_simple() {
    let graph_json = r#"{
        "name": "test-graph",
        "services": {
            "api": { "image": "my-api" }
        }
    }"#;
    let backend = Arc::new(MockBackend::new("mock"));
    let engine = WorkloadGraphEngine::new(backend.clone());

    let _handle_id = engine.run(graph_json, "{}").await.unwrap();

    let calls = backend.calls.lock().unwrap();
    assert!(calls.iter().any(|c| matches!(c, perry_container_compose::testing::mock_backend::RecordedCall::Run(_))));
}
