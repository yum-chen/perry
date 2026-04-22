//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use super::get_global_backend;
use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

/// Find a resource file in the standard location.
fn find_resource(filename: &str) -> Option<PathBuf> {
    // 1. Check relative to current directory
    let rel = PathBuf::from("res").join("container").join(filename);
    if rel.exists() {
        return Some(rel);
    }

    // 2. Check relative to executable
    if let Ok(mut exe) = std::env::current_exe() {
        exe.pop(); // binary dir
        let res = exe.join("res").join("container").join(filename);
        if res.exists() {
            return Some(res);
        }
        exe.pop(); // workspace root maybe
        let res = exe.join("res").join("container").join(filename);
        if res.exists() {
            return Some(res);
        }
    }

    None
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
        ports: None,
        volumes: None,
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ..Default::default()
    };

    let backend = Arc::clone(get_global_backend().await?);

    // Requirement 13.3: Run with seccomp profile blocking dangerous syscalls.
    let seccomp_path = find_resource("seccomp-restrictive.json")
        .ok_or_else(|| ContainerError::BackendError {
            code: -1,
            message: "Critical security resource 'seccomp-restrictive.json' not found".to_string()
        })?;

    let seccomp_json = std::fs::read_to_string(seccomp_path)
        .map_err(|e| ContainerError::BackendError {
            code: -1,
            message: format!("Failed to read seccomp profile: {}", e)
        })?;

    let profile = super::backend::SecurityProfile {
        seccomp: Some(seccomp_json),
        readonly_rootfs: true,
    };
    let handle = backend.run_with_security(&spec, &profile).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;

    backend.logs(&handle.id, None).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })
}
