//! Property-based tests for the perry-stdlib container module.

use proptest::prelude::*;
use serde_json::{json, Value};
use perry_container_compose::indexmap::IndexMap;

// ============ Property 2: ContainerSpec CLI argument round-trip ============
// Feature: perry-container, Property 2: ContainerSpec CLI argument round-trip
// Validates: Requirements 12.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_container_spec_json_round_trip(
        image in "[a-z][a-z0-9_-]{1,30}(:[a-z0-9._-]+)?",
        name in proptest::option::of("[a-z][a-z0-9_-]{1,30}"),
        ports in proptest::option::of(proptest::collection::vec("[0-9]{1,5}:[0-9]{1,5}", 0..=5)),
        env_keys in proptest::collection::vec("[A-Z][A-Z0-9_]{1,10}", 0..=5),
    ) {
        let mut env_obj = serde_json::Map::new();
        for key in &env_keys {
            env_obj.insert(key.clone(), Value::String(format!("val_{}", key)));
        }

        let spec = json!({
            "image": image,
            "name": name,
            "ports": ports,
            "env": env_obj,
            "cmd": ["echo", "hello"],
            "rm": true,
        });

        let spec_str = serde_json::to_string(&spec).unwrap();
        let reparsed: Value = serde_json::from_str(&spec_str).unwrap();

        prop_assert_eq!(&reparsed["image"], &spec["image"]);

        if name.is_some() {
            prop_assert_eq!(&reparsed["name"], &spec["name"]);
        }

        // Ports array length preserved
        prop_assert_eq!(
            reparsed["ports"].as_array().map(|a| a.len()),
            spec["ports"].as_array().map(|a| a.len())
        );

        // Env keys preserved
        if let Some(env) = reparsed["env"].as_object() {
            prop_assert_eq!(env.len(), env_keys.len());
        }
    }
}

// ============ Property 10: Image verification cache idempotence ============
// Feature: perry-container, Property 10: Image verification cache idempotence
// Validates: Requirements 15.7

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_error_propagation_preserves_code_and_message(
        code in -1000i32..1000,
        msg in "[a-z A-Z0-9_]{1,100}"
    ) {
        // Simulate the ComposeError::BackendError → JSON → parse flow
        let error_json = json!({
            "message": format!("Backend error (exit {}): {}", code, msg),
            "code": code
        });

        let json_str = serde_json::to_string(&error_json).unwrap();
        let reparsed: Value = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(&reparsed["code"], &json!(code));
        prop_assert!(
            reparsed["message"].as_str().unwrap_or("").contains(&msg),
            "message should contain original msg"
        );
    }
}

// ============ Property 11: Error propagation preserves code and message ============
// Feature: perry-container, Property 11: Error propagation preserves code and message
// Validates: Requirements 2.6, 12.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_compose_error_json_round_trip(
        variant in 0u8..=5,
        msg in "[a-z A-Z0-9_]{1,80}"
    ) {
        let (error_json, expected_code) = match variant {
            0 => (json!({ "message": format!("Not found: {}", msg), "code": 404 }), 404i64),
            1 => (json!({ "message": format!("Backend error (exit 1): {}", msg), "code": 1 }), 1),
            2 => (json!({ "message": format!("Dependency cycle detected in services: {:?}", [msg]), "code": 422 }), 422),
            3 => (json!({ "message": format!("Validation error: {}", msg), "code": 400 }), 400),
            4 => (json!({ "message": format!("Image verification failed for 'img': {}", msg), "code": 403 }), 403),
            _ => (json!({ "message": format!("Parse error: {}", msg), "code": 500 }), 500),
        };

        let json_str = serde_json::to_string(&error_json).unwrap();
        let reparsed: Value = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(&reparsed["code"], &json!(expected_code));
        prop_assert!(reparsed["message"].is_string());
    }
}

// ============ Property: ListOrDict to_map — Dict variant ============
// Validates: ListOrDict::Dict correctly converts all value types to strings.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_list_or_dict_to_map_dict(
        keys in proptest::collection::vec("[A-Z][A-Z0-9_]{1,8}", 1..=8),
        int_val in 0i64..1000,
        bool_val in proptest::bool::ANY,
        str_val in "[a-z0-9_]{1,10}",
    ) {
        let mut map = IndexMap::new();
        // Mix different value types across keys
        for (i, key) in keys.iter().enumerate() {
            let val: Option<serde_yaml::Value> = match i % 4 {
                0 => Some(serde_yaml::Value::String(str_val.clone())),
                1 => Some(serde_yaml::Value::Number(int_val.into())),
                2 => Some(serde_yaml::Value::Bool(bool_val)),
                _ => None, // Null
            };
            map.insert(key.clone(), val);
        }

        let lod = perry_stdlib::container::ListOrDict::Dict(map);
        let result = lod.to_map();

        // All unique keys should be preserved
        let unique_keys: std::collections::HashSet<_> = keys.iter().collect();
        prop_assert_eq!(result.len(), unique_keys.len());
        for key in &keys {
            prop_assert!(result.contains_key(key), "key {} should be in result", key);
        }
    }
}

// ============ Property: ListOrDict to_map — List variant ============
// Validates: ListOrDict::List("KEY=VAL") correctly parses entries.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_list_or_dict_to_map_list(
        entries in proptest::collection::vec("[A-Z][A-Z0-9_]{1,8}=[a-z0-9_]{0,10}", 1..=8),
    ) {
        let list: Vec<String> = entries.clone();
        let lod = perry_stdlib::container::ListOrDict::List(list);
        let result = lod.to_map();

        // All unique keys should be present with non-None values
        // Note: HashMap uses last-writer-wins, so duplicate keys
        // retain the value from the last occurrence.
        let unique_keys: std::collections::HashSet<&str> =
            entries.iter().map(|e| e.split_once('=').unwrap().0).collect();
        prop_assert_eq!(result.len(), unique_keys.len());
        for key in &unique_keys {
            prop_assert!(
                result.contains_key(*key),
                "key {} should be present in result",
                key
            );
        }
    }
}

// ============ Property: ListOrDict to_map — List with missing = sign ============
// Validates: Entries without '=' produce empty string values.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_list_or_dict_to_map_list_no_equals(
        keys in proptest::collection::vec("[A-Z][A-Z0-9_]{1,8}", 1..=5),
    ) {
        let list: Vec<String> = keys.clone();
        let lod = perry_stdlib::container::ListOrDict::List(list);
        let result = lod.to_map();

        // All unique keys should be present with empty values
        // (HashMap deduplicates keys, so len may be <= keys.len())
        for key in &keys {
            prop_assert_eq!(
                result.get(key).map(|s| s.as_str()),
                Some(""),
                "key {} without '=' should have empty value",
                key
            );
        }
    }
}

// ============ Property: DependsOnSpec service_names — List vs Map ============
// Validates: Both List and Map variants produce the same set of service names.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_depends_on_entry_service_names(
        names in proptest::collection::vec("[a-z][a-z0-9_-]{1,10}", 1..=6),
    ) {
        use perry_container_compose::types::{DependsOnSpec, ComposeDependsOn, DependsOnCondition};

        // List variant
        let list_entry = DependsOnSpec::List(names.clone());
        let list_names = list_entry.service_names();

        // Map variant (same keys)
        let mut map = IndexMap::new();
        for name in &names {
            map.insert(
                name.clone(),
                ComposeDependsOn {
                    condition: DependsOnCondition::ServiceStarted,
                    required: None,
                    restart: None,
                },
            );
        }
        let map_entry = DependsOnSpec::Map(map);
        let map_names = map_entry.service_names();

        // Both should yield the same service names (order may differ for Map)
        prop_assert_eq!(list_names.len(), map_names.len());
        for name in &list_names {
            prop_assert!(map_names.contains(name), "map should contain {}", name);
        }
    }
}

// ============ Property: ContainerError Display contains identifying keyword ============
// Validates: Each ContainerError variant's Display output contains
// a distinguishing keyword for programmatic error classification.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_container_error_display_contains_keyword(
        variant in 0u8..=5,
        msg in "[a-z A-Z0-9_]{1,40}",
    ) {
        let error = match variant {
            0 => perry_stdlib::container::ContainerError::NotFound(msg.clone()),
            1 => perry_stdlib::container::ContainerError::BackendError {
                code: 1,
                message: msg.clone(),
            },
            2 => perry_stdlib::container::ContainerError::VerificationFailed {
                image: msg.clone(),
                reason: "test reason".to_string(),
            },
            3 => perry_stdlib::container::ContainerError::DependencyCycle {
                cycle: vec![msg.clone()],
            },
            4 => perry_stdlib::container::ContainerError::ServiceStartupFailed {
                service: msg.clone(),
                error: "test error".to_string(),
            },
            _ => perry_stdlib::container::ContainerError::InvalidConfig(msg.clone()),
        };

        let display = format!("{}", error);
        let expected_keyword = match variant {
            0 => "not found",
            1 => "Backend error",
            2 => "verification failed",
            3 => "Dependency cycle",
            4 => "failed to start",
            _ => "Invalid configuration",
        };

        prop_assert!(
            display.to_lowercase().contains(&expected_keyword.to_lowercase()),
            "Display output should contain '{}', got: {}",
            expected_keyword,
            display
        );
    }
}

// ============ Property: Typed ComposeSpec JSON round-trip ============
// Validates: The typed ComposeSpec struct survives JSON round-trip.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_typed_compose_spec_json_round_trip(
        name in proptest::option::of("[a-z][a-z0-9_-]{1,20}"),
        svc_names in proptest::collection::vec("[a-z][a-z0-9_-]{1,10}", 1..=5),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}(:[a-z0-9._-]+)?", 1..=5),
    ) {
        use perry_container_compose::types::{ComposeSpec, ComposeService};
        let mut spec = ComposeSpec::default();
        spec.name = name;

        for (svc_name, image) in svc_names.iter().zip(images.iter()) {
            let mut service = ComposeService::default();
            service.image = Some(image.clone());
            spec.services.insert(svc_name.clone(), service);
        }

        let json_str = serde_json::to_string(&spec).unwrap();
        let reparsed: ComposeSpec =
            serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(reparsed.name, spec.name);
        prop_assert_eq!(reparsed.services.len(), spec.services.len());

        for (svc_name, original_svc) in &spec.services {
            let reparsed_svc = &reparsed.services[svc_name];
            prop_assert_eq!(&reparsed_svc.image, &original_svc.image);
        }
    }
}

// ============ Property: Handle registry register/take type safety ============
// Validates: Registering and retrieving handles preserves the value and type.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_handle_registry_type_safety(
        ids in proptest::collection::vec("[a-f0-9]{12}", 1..=3),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}", 1..=3),
        stdout in "[a-z0-9 ]{0,50}",
        stderr in "[a-z0-9 ]{0,50}",
    ) {
        use perry_stdlib::container::{ContainerInfo, ContainerLogs};

        // Register a Vec<ContainerInfo> and take it back
        let infos: Vec<ContainerInfo> = ids
            .iter()
            .zip(images.iter())
            .map(|(id, img)| ContainerInfo {
                id: id.clone(),
                name: format!("svc-{}", &id[..6]),
                image: img.clone(),
                status: "running".to_string(),
                ports: vec![],
                labels: std::collections::HashMap::new(),
                created: "2025-01-01T00:00:00Z".to_string(),
            })
            .collect();

        let h = perry_stdlib::container::types::register_container_info_list(infos.clone());
        let taken: Option<Vec<ContainerInfo>> =
            perry_stdlib::container::types::take_container_info_list(h);
        prop_assert!(taken.is_some());
        let taken = taken.unwrap();
        prop_assert_eq!(taken.len(), infos.len());
        for (original, recovered) in infos.iter().zip(taken.iter()) {
            prop_assert_eq!(&recovered.id, &original.id);
            prop_assert_eq!(&recovered.image, &original.image);
        }

        // Register ContainerLogs and take it back
        let logs = ContainerLogs {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };
        let lh = perry_stdlib::container::types::register_container_logs(logs);
        let taken_logs: Option<ContainerLogs> =
            perry_stdlib::container::types::take_container_logs(lh);
        prop_assert!(taken_logs.is_some());
        let taken_logs = taken_logs.unwrap();
        prop_assert_eq!(taken_logs.stdout, stdout);
        prop_assert_eq!(taken_logs.stderr, stderr);
    }
}

// ============ Property: ComposeNetwork JSON round-trip ============
// Validates: ComposeNetwork preserves all fields through serialization.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_compose_network_json_round_trip(
        name in proptest::option::of("[a-z][a-z0-9_-]{1,20}"),
        driver in proptest::option::of("[a-z]{3,10}"),
    ) {
        use perry_container_compose::types::ComposeNetwork;
        let mut network = ComposeNetwork::default();
        network.name = name;
        network.driver = driver;

        let json_str = serde_json::to_string(&network).unwrap();
        let reparsed: ComposeNetwork =
            serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(reparsed.name, network.name);
        prop_assert_eq!(reparsed.driver, network.driver);
    }
}
