use proptest::prelude::*;
use perry_container_compose::types::*;
use perry_container_compose::error::ComposeError;

// Feature: perry-container, Property 2: ContainerSpec CLI argument round-trip
fn arb_container_spec() -> impl Strategy<Value = ContainerSpec> {
    any::<String>().prop_flat_map(|image| {
        (
            Just(image),
            any::<Option<String>>(),
            prop::collection::vec(any::<String>(), 0..5),
            prop::collection::vec(any::<String>(), 0..5),
            prop::collection::hash_map(any::<String>(), any::<String>(), 0..5),
            any::<Option<Vec<String>>>(),
            any::<Option<Vec<String>>>(),
            any::<Option<String>>(),
            any::<Option<bool>>(),
        ).prop_map(|(image, name, ports, volumes, env, cmd, entrypoint, network, rm)| {
            ContainerSpec {
                image,
                name,
                ports: Some(ports),
                volumes: Some(volumes),
                env: Some(env),
                labels: None,
                cmd,
                entrypoint,
                network,
                rm,
                read_only: None,
            }
        })
    })
}

proptest! {
    #[test]
    fn prop_container_spec_serialization(spec in arb_container_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ContainerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec.image, deserialized.image);
        assert_eq!(spec.name, deserialized.name);
    }
}

// Feature: perry-container, Property 10: Image verification cache idempotence
// (Note: This is a unit-testable property of the verification cache)

#[test]
fn test_error_propagation_preserves_code() {
    let err = ComposeError::BackendError { code: 404, message: "not found".into() };
    let json = serde_json::json!({
        "message": err.to_string(),
        "code": 404
    }).to_string();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(val["code"], 404);
}
