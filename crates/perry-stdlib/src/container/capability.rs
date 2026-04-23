use super::types::{ContainerLogs, ComposeError};
use super::verification;
use super::backend::get_global_backend_instance;
use perry_container_compose::types::ContainerSpec;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<std::collections::HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, ComposeError> {
    let digest = verification::verify_image(image).await
        .map_err(|e| ComposeError::VerificationFailed { image: image.into(), reason: e.to_string() })?;

    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        ports: None,
        volumes: None,
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        read_only: Some(true),
        entrypoint: None,
    };

    let backend = get_global_backend_instance().await
        .map_err(|e| ComposeError::BackendNotAvailable { name: "global".into(), reason: e.to_string() })?;

    let handle = backend.run(&spec).await?;
    let logs = backend.logs(&handle.id, None).await?;

    Ok(ContainerLogs {
        stdout: logs.stdout,
        stderr: logs.stderr,
    })
}
