//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use super::get_global_backend;
use std::collections::HashMap;
use std::sync::Arc;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, ContainerError> {
    let digest = verification::verify_image(image).await?;

    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        ports: Some(vec![]),
        volumes: Some(vec![]),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ..Default::default()
    };

    let backend = Arc::clone(get_global_backend().await?);
    let handle = backend.run(&spec).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;

    backend.logs(&handle.id, None).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })
}
