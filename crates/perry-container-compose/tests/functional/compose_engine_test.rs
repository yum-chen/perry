use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::testing::mock_backend::{MockBackend, RecordedCall};
use perry_container_compose::types::{ComposeSpec, ComposeService, ContainerHandle};
use std::sync::Arc;

#[tokio::test]
async fn test_compose_up_order() {
    let mock = MockBackend::new();
    let backend = Arc::new(mock);

    let mut services = indexmap::IndexMap::new();
    services.insert("db".to_string(), ComposeService {
        image: Some("postgres".to_string()),
        ..Default::default()
    });
    services.insert("web".to_string(), ComposeService {
        image: Some("nginx".to_string()),
        depends_on: Some(perry_container_compose::types::DependsOnSpec::List(vec!["db".to_string()])),
        ..Default::default()
    });

    let spec = ComposeSpec {
        services,
        ..Default::default()
    };

    backend.push_response(Ok(ContainerHandle { id: "db-id".into(), name: None }));
    backend.push_response(Ok(ContainerHandle { id: "web-id".into(), name: None }));

    let engine = ComposeEngine::new(spec, "test-proj".into(), backend.clone());
    engine.up(&[], true, false, false).await.unwrap();

    let calls = backend.calls.lock().unwrap();
    // Expected order: db, then web
    match (&calls[0], &calls[1]) {
        (RecordedCall::Run(db_spec), RecordedCall::Run(web_spec)) => {
            assert!(db_spec.image == "postgres");
            assert!(web_spec.image == "nginx");
        },
        _ => panic!("Unexpected call order: {:?}", calls),
    }
}
