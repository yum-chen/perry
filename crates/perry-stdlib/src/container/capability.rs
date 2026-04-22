use super::types::{ContainerSpec, ContainerLogs, ComposeError};
use super::verification;
use super::get_global_backend_instance;
use std::collections::HashMap;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, ComposeError> {
    // 1. Verify image signature before running
    let digest = verification::verify_image(image).await?;

    // 2. Build ephemeral ContainerSpec with security constraints
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        volumes: None,
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        read_only: Some(true),
        ..Default::default()
    };

    // 3. Run and collect output
    let backend = get_global_backend_instance().await.map_err(|e| ComposeError::BackendNotAvailable { name: "global".into(), reason: e })?;
    backend.run(&spec).await?;
    backend.logs(spec.name.as_ref().unwrap(), None).await
}
