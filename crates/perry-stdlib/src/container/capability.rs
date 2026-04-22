//! OCI isolation for Shell capabilities.

use std::collections::HashMap;
use crate::container::types::{ContainerLogs};
use crate::container::verification;
use crate::container::context::ContainerContext;
use perry_container_compose::types::ContainerSpec;

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
    let backend = ContainerContext::global().get_backend().await.map_err(|e| e.to_string())?;

    // Create security profile for sandboxed capability
    let profile = crate::container::backend::SecurityProfile {
        read_only_rootfs: true,
        seccomp_profile: None, // Use default
        cap_drop: vec!["ALL".to_string()],
    };

    let handle = backend.run_with_security(&spec, &profile).await.map_err(|e| e.to_string())?;

    // 4. Wait for exit and collect logs
    let logs = backend.wait_and_logs(&handle.id).await.map_err(|e| e.to_string())?;

    Ok(ContainerLogs {
        stdout: logs.stdout,
        stderr: logs.stderr,
    })
}
