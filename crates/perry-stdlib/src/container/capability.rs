//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerLogs, ContainerSpec};
use super::verification;
use super::mod_priv::get_global_backend_instance;
use std::collections::HashMap;
use perry_container_compose::error::ComposeError;

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
    let backend = get_global_backend_instance().await;
    let digest = verification::verify_image(image, &backend).await?;

    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    let handle = backend.run(&spec.into()).await?;
    let logs: perry_container_compose::types::ContainerLogs = backend.logs(&handle.id, None).await?;

    Ok(logs.into())
}
