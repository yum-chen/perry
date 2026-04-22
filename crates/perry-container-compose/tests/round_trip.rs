use perry_container_compose::types::{ComposeSpec, ComposeService};
use perry_container_compose::compose::ComposeEngine;
use proptest::prelude::*;

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
proptest! {
    #[test]
    fn prop_compose_spec_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&deserialized).unwrap();
        prop_assert_eq!(json, json2);
    }
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
proptest! {
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_with_dag()) {
        let order = ComposeEngine::resolve_startup_order(&spec).unwrap();
        let pos: std::collections::HashMap<&str, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.as_str(), i)).collect();
        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(pos[dep.as_str()] < pos[name.as_str()],
                        "dep {} should come before {}", dep, name);
                }
            }
        }
    }
}

// Feature: perry-container, Property 4: Cycle detection is complete
proptest! {
    #[test]
    fn prop_cycle_detection(spec in arb_compose_spec_with_cycle()) {
        let result = ComposeEngine::resolve_startup_order(&spec);
        match result {
            Err(perry_container_compose::error::ComposeError::DependencyCycle { services }) => {
                prop_assert!(!services.is_empty());
            }
            _ => prop_assert!(false, "Expected DependencyCycle error"),
        }
    }
}

fn arb_compose_spec() -> impl Strategy<Value = ComposeSpec> {
    // Avoid '$' to prevent interpolation from mangling the round-trip test.
    prop::option::of("[a-zA-Z0-9._/-]{1,20}").prop_flat_map(|name| {
        prop::collection::vec(arb_service(), 1..5).prop_map(move |services| {
            let mut spec = ComposeSpec::default();
            spec.name = name.clone();
            for (i, svc) in services.into_iter().enumerate() {
                spec.services.insert(format!("svc-{}", i), svc);
            }
            spec
        })
    })
}

fn arb_service() -> impl Strategy<Value = ComposeService> {
    // Avoid '$' to prevent interpolation from mangling the round-trip test.
    // Also avoid chars that might trigger YAML special parsing like ':', '[', '{' etc. in simple strings.
    prop::option::of("[a-zA-Z0-9._/-]{1,20}").prop_map(|image| {
        let mut svc = ComposeService::default();
        svc.image = image;
        svc
    })
}

fn arb_compose_spec_with_dag() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec(arb_service(), 1..5).prop_map(|services| {
        let mut spec = ComposeSpec::default();
        let mut prev: Option<String> = None;
        for (i, svc) in services.into_iter().enumerate() {
            let name = format!("svc-{}", i);
            let mut svc = svc;
            if let Some(p) = prev {
                svc.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec![p]));
            }
            spec.services.insert(name.clone(), svc);
            prev = Some(name);
        }
        spec
    })
}

// Feature: perry-container, Property 5: YAML round-trip preserves ComposeSpec
proptest! {
    #[test]
    fn prop_yaml_round_trip(spec in arb_compose_spec()) {
        let yaml = serde_yaml::to_string(&spec).unwrap();
        let env = std::collections::HashMap::new();
        let deserialized = perry_container_compose::yaml::parse_compose_yaml(&yaml, &env).unwrap();
        let yaml2 = serde_yaml::to_string(&deserialized).unwrap();
        prop_assert_eq!(yaml, yaml2);
    }
}

// Feature: perry-container, Property 6: Environment variable interpolation correctness
proptest! {
    #[test]
    fn prop_interpolation(
        key in "[A-Z][A-Z0-9_]{1,10}", // Key must be non-empty
        val in "[a-z0-9_]{1,20}",     // Value non-empty for simplicity in basic test
        default in "[a-z0-9_]{1,20}"
    ) {
        let mut env = std::collections::HashMap::new();
        env.insert(key.clone(), val.clone());

        // ${VAR}
        let input1 = format!("prefix-${{{}}}-suffix", key);
        prop_assert_eq!(perry_container_compose::yaml::interpolate(&input1, &env), format!("prefix-{}-suffix", val));

        // $VAR (no braces)
        let input1b = format!("prefix-${}-suffix", key);
        prop_assert_eq!(perry_container_compose::yaml::interpolate(&input1b, &env), format!("prefix-{}-suffix", val));

        // ${VAR:-default} when set
        let input2 = format!("prefix-${{{}:-default}}-suffix", key);
        prop_assert_eq!(perry_container_compose::yaml::interpolate(&input2, &env), format!("prefix-{}-suffix", val));

        // ${VAR:-default} when missing
        let mut empty_env = std::collections::HashMap::new();
        // Ensure the key is NOT in process env to avoid contamination
        // We use a prefix that's unlikely to exist
        let unique_key = format!("NONEXISTENT_VAR_{}", key);
        let input3 = format!("prefix-${{{}:-{}}}-suffix", unique_key, default);
        prop_assert_eq!(perry_container_compose::yaml::interpolate(&input3, &empty_env), format!("prefix-{}-suffix", default));

        // $$ escape
        prop_assert_eq!(perry_container_compose::yaml::interpolate("$$VAR", &env), "$VAR");
    }
}

// Feature: perry-container, Property 7: Compose file merge is last-writer-wins
proptest! {
    #[test]
    fn prop_merge_last_writer_wins(
        image1 in "[a-z]{5,10}",
        image2 in "[a-z]{5,10}"
    ) {
        let mut spec1 = ComposeSpec::default();
        let mut svc1 = ComposeService::default();
        svc1.image = Some(image1.clone());
        spec1.services.insert("web".into(), svc1);

        let mut spec2 = ComposeSpec::default();
        let mut svc2 = ComposeService::default();
        svc2.image = Some(image2.clone());
        spec2.services.insert("web".into(), svc2);

        spec1.merge(spec2);

        prop_assert_eq!(spec1.services["web"].image.as_ref().unwrap(), &image2);
    }
}

fn arb_compose_spec_with_cycle() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec(arb_service(), 2..3).prop_map(|services| {
        let mut spec = ComposeSpec::default();
        let name0 = "svc-0".to_string();
        let name1 = "svc-1".to_string();

        let mut svc0 = services[0].clone();
        svc0.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec![name1.clone()]));

        let mut svc1 = services[0].clone(); // image from first one
        svc1.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec![name0.clone()]));

        spec.services.insert(name0, svc0);
        spec.services.insert(name1, svc1);
        spec
    })
}
