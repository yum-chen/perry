use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec, DependsOnCondition, ComposeDependsOn};
use perry_container_compose::compose::resolve_startup_order;
use proptest::prelude::*;
use std::collections::HashMap;
use indexmap::IndexMap;

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
proptest! {
    #[test]
    fn prop_compose_spec_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();

        // We compare JSON strings because direct Eq on IndexMap/Spec is complex with extensions
        let json2 = serde_json::to_string(&deserialized).unwrap();
        prop_assert_eq!(json, json2);
    }
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
proptest! {
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_with_dag()) {
        let order = resolve_startup_order(&spec).unwrap();
        let pos: HashMap<String, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.clone(), i)).collect();

        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(pos[&dep] < pos[name],
                        "dep {} (pos {}) should come before {} (pos {})", dep, pos[&dep], name, pos[name]);
                }
            }
        }
    }
}

// Feature: perry-container, Property 4: Cycle detection is complete
proptest! {
    #[test]
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_with_cycle()) {
        let res = resolve_startup_order(&spec);
        prop_assert!(res.is_err(), "Should detect cycle in {:?}", spec);
        match res.unwrap_err() {
            perry_container_compose::error::ComposeError::DependencyCycle { services } => {
                prop_assert!(!services.is_empty());
            }
            _ => prop_assert!(false, "Expected DependencyCycle error"),
        }
    }
}

// Generators

fn arb_service_name() -> impl Strategy<Value = String> {
    "[a-z0-9]{1,10}"
}

fn arb_compose_service() -> impl Strategy<Value = ComposeService> {
    any::<Option<String>>().prop_map(|image| ComposeService {
        image,
        ..Default::default()
    })
}

fn arb_compose_spec() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec((arb_service_name(), arb_compose_service()), 1..5)
        .prop_map(|services| {
            let mut map = IndexMap::new();
            for (name, svc) in services {
                map.insert(name, svc);
            }
            ComposeSpec {
                services: map,
                ..Default::default()
            }
        })
}

fn arb_compose_spec_with_dag() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec(arb_service_name(), 2..10).prop_flat_map(|names| {
        let n = names.len();
        // Generate dependencies such that higher index depends on lower index (DAG)
        let mut services = IndexMap::new();
        for (i, name) in names.iter().enumerate() {
            let mut svc = ComposeService {
                image: Some("alpine".to_string()),
                ..Default::default()
            };
            if i > 0 {
                // Pick some random subset of previous nodes as dependencies
                let deps_count = (i as u32) % 3; // 0, 1, or 2 deps
                if deps_count > 0 {
                    let mut dep_names = Vec::new();
                    for j in 0..i {
                        if dep_names.len() < deps_count as usize {
                            dep_names.push(names[j].clone());
                        }
                    }
                    svc.depends_on = Some(DependsOnSpec::List(dep_names));
                }
            }
            services.insert(name.clone(), svc);
        }
        Just(ComposeSpec { services, ..Default::default() })
    })
}

fn arb_compose_spec_with_cycle() -> impl Strategy<Value = ComposeSpec> {
    Just(()).prop_flat_map(|_| {
        let mut services = IndexMap::new();
        services.insert("a".to_string(), ComposeService {
            image: Some("alpine".to_string()),
            depends_on: Some(DependsOnSpec::List(vec!["b".to_string()])),
            ..Default::default()
        });
        services.insert("b".to_string(), ComposeService {
            image: Some("alpine".to_string()),
            depends_on: Some(DependsOnSpec::List(vec!["a".to_string()])),
            ..Default::default()
        });
        Just(ComposeSpec { services, ..Default::default() })
    })
}
