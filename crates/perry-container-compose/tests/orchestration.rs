use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeService};
use std::sync::Arc;

mod common;
use common::MockBackend;

#[tokio::test]
async fn test_compose_up_success() {
    let mut spec = ComposeSpec::default();
    spec.services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        ..Default::default()
    });
    spec.services.insert("db".into(), ComposeService {
        image: Some("postgres".into()),
        ..Default::default()
    });

    let backend = Arc::new(MockBackend::default());
    let engine = ComposeEngine::new(spec, "test-project".into(), backend.clone());

    let handle = engine.up(&[], true, false, false).await.expect("up failed");

    assert_eq!(handle.project_name, "test-project");
    assert_eq!(handle.services.len(), 2);

    let state = backend.state.lock().unwrap();
    assert_eq!(state.containers.len(), 2);
    // Check order: db then web (alphabetical since no deps)
    assert!(state.actions[0].starts_with("run:db"));
    assert!(state.actions[1].starts_with("run:web"));
}

#[tokio::test]
async fn test_compose_up_rollback_on_failure() {
    let mut spec = ComposeSpec::default();
    spec.services.insert("db".into(), ComposeService {
        image: Some("postgres".into()),
        ..Default::default()
    });
    spec.services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        ..Default::default()
    });

    let backend = Arc::new(MockBackend::default());
    {
        let mut state = backend.state.lock().unwrap();
        state.fail_on_run = Some("web".into());
    }

    let engine = ComposeEngine::new(spec, "fail-project".into(), backend.clone());
    let result = engine.up(&[], true, false, false).await;

    assert!(result.is_err());

    let state = backend.state.lock().unwrap();
    // Should have started db, tried web, then stopped/removed db
    assert!(state.containers.is_empty());

    let actions: Vec<_> = state.actions.iter().map(|s| s.split(':').next().unwrap()).collect();
    assert!(actions.contains(&"run"));    // db
    assert!(actions.contains(&"stop"));   // db rollback
    assert!(actions.contains(&"remove")); // db rollback
}

#[tokio::test]
async fn test_compose_down_cleans_resources() {
    let mut spec = ComposeSpec::default();
    spec.services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        ..Default::default()
    });

    let backend = Arc::new(MockBackend::default());
    let engine = ComposeEngine::new(spec, "down-project".into(), backend.clone());

    let handle = engine.up(&[], true, false, false).await.unwrap();
    engine.down(&handle.services, false, true).await.expect("down failed");

    let state = backend.state.lock().unwrap();
    assert!(state.containers.is_empty(), "Containers should be empty, but found: {:?}", state.containers);
    assert!(state.networks.is_empty());
    assert!(state.volumes.is_empty());
}
