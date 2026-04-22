//! OCI isolation for Shell capabilities.

use std::collections::HashMap;
use crate::container::types::{ContainerSpec, ContainerLogs};
use crate::container::verification;
use crate::container::context::ContainerContext;

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
    // 1. Verify image
    let _digest = verification::verify_image(image).await?;

    // 2. Build spec
    let spec = ContainerSpec {
        image: image.to_string(),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    // 3. Run
    let ctx = ContainerContext::global();
    let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
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
    }).await.map_err(|e| e.to_string())?;

    // 4. Logs (simplified: wait for completion should be here)
    let logs = backend.logs(&handle.id, None).await.map_err(|e| e.to_string())?;

    Ok(ContainerLogs {
        stdout: logs.stdout,
        stderr: logs.stderr,
    })
}
