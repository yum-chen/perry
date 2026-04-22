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
    any::<Option<String>>().prop_flat_map(|name| {
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
    any::<Option<String>>().prop_map(|image| {
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

// Feature: perry-container, Property 6: Environment variable interpolation correctness
#[test]
fn test_env_interpolation_manual() {
    let mut env = std::collections::HashMap::new();
    env.insert("VAR".to_string(), "val".to_string());
    env.insert("EMPTY".to_string(), "".to_string());

    assert_eq!(perry_container_compose::yaml::interpolate_yaml("hello ${VAR}", &env), "hello val");
    assert_eq!(perry_container_compose::yaml::interpolate_yaml("hello ${MISSING:-default}", &env), "hello default");
    assert_eq!(perry_container_compose::yaml::interpolate_yaml("hello ${EMPTY:-default}", &env), "hello default");
    assert_eq!(perry_container_compose::yaml::interpolate_yaml("hello ${VAR:-default}", &env), "hello val");
}

// Feature: perry-container, Property 2: ContainerSpec CLI argument round-trip
#[test]
fn test_container_spec_to_docker_args() {
    use perry_container_compose::types::ContainerSpec;
    use perry_container_compose::backend::DockerProtocol;
    use perry_container_compose::backend::CliProtocol;

    let mut env = std::collections::HashMap::new();
    env.insert("K".to_string(), "V".to_string());

    let spec = ContainerSpec {
        image: "nginx".into(),
        name: Some("my-nginx".into()),
        ports: Some(vec!["8080:80".into()]),
        env: Some(env),
        ..Default::default()
    };

    let protocol = DockerProtocol;
    let args = protocol.run_args(&spec);

    let args_str = args.join(" ");
    assert!(args_str.contains("run -d"));
    assert!(args_str.contains("--name my-nginx"));
    assert!(args_str.contains("-p 8080:80"));
    assert!(args_str.contains("-e K=V"));
    assert!(args_str.contains("nginx"));
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
