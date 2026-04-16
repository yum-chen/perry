//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use super::get_global_backend;
use crate::container::backend::SecurityProfile;
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
    // 1. Verify image before running
    let digest = verification::verify_image(image).await?;

    // 2. Build ephemeral ContainerSpec with security constraints
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        ports: Some(vec![]),
        volumes: Some(vec![]),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true), // Always remove on exit
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ..Default::default()
    };

    let backend = Arc::clone(get_global_backend().await?);

    // 3. Run with security profile
    let handle = backend.run_with_security(&spec, &SecurityProfile::default()).await
        .map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;

    // 4. Wait for completion and collect output
    backend.wait_and_logs(&handle.id).await
        .map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })
}
