//! OCI isolation for Shell capabilities.

use std::collections::HashMap;
use crate::container::types::{ContainerSpec, ContainerLogs};
use crate::container::verification;
use crate::container::mod_private::get_global_backend_instance;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, String> {
    // 1. Verify image signature before running
    let digest = verification::verify_image(image).await?;

    // 2. Build ephemeral ContainerSpec with security constraints
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        // No persistent volumes
        volumes: None,
        // No network access by default (unless grants.network == true)
        network: if grants.network { None } else { Some("none".to_string()) },
        // Read-only root filesystem
        rm: Some(true),  // Always remove on exit
        read_only: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    // 3. Run
    let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
    let handle = backend.run(&perry_container_compose::types::ContainerSpec {
        image: spec.image,
        name: spec.name,
        ports: spec.ports,
        volumes: spec.volumes,
        env: spec.env,
        cmd: spec.cmd,
        entrypoint: spec.entrypoint,
        network: spec.network,
        rm: spec.rm,
        read_only: spec.read_only,
        labels: spec.labels,
        seccomp: spec.seccomp,
    }).await.map_err(|e| e.to_string())?;

    // 4. Wait for completion and collect output
    let _ = backend.wait(&handle.id).await.map_err(|e| e.to_string())?;
    let logs = backend.logs(&handle.id, None).await.map_err(|e| e.to_string())?;

    // 5. Container is auto-removed (rm: true)
    Ok(ContainerLogs {
        stdout: logs.stdout,
        stderr: logs.stderr,
    })
}
