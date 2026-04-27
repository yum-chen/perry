//! Service runtime state and name generation.

use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::{ComposeService, ContainerSpec};
use md5::{Digest, Md5};
use std::sync::atomic::{AtomicU32, Ordering};

/// Global counter for deterministic random suffixes in debug mode.
static DEBUG_RANDOM_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Generate a unique container name for a service.
///
/// Format: {md5_8chars}-{random_hex8}
/// e.g. a3f2b1c9-00e4f2a1
pub fn generate_name(input: &str) -> String {
    // MD5 hash of the input (service YAML or image name) for a stable prefix
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);
    let short_hash = &hash_str[..8];

    // Use an atomic counter in debug mode to avoid collisions during development
    // while maintaining predictability for tests and logs.
    let random_suffix: u32 = if cfg!(debug_assertions) {
        DEBUG_RANDOM_COUNTER.fetch_add(1, Ordering::SeqCst)
    } else {
        rand::random()
    };

    format!("{}-{:08x}", short_hash, random_suffix)
}

/// Service runtime state tracking.
pub struct ServiceState {
    /// Container ID
    pub container_id: String,
    /// Container name
    pub container_name: String,
    /// Whether the service container is running
    pub running: bool,
}

impl ServiceState {
    /// Create a service state from an explicit container name.
    pub fn new(container_id: String, container_name: String, running: bool) -> Self {
        ServiceState {
            container_id,
            container_name,
            running,
        }
    }
}

/// Generate a container name for a service, using explicit name if set.
pub fn service_container_name(svc: &ComposeService, service_name: &str) -> String {
    if let Some(explicit) = svc.explicit_name() {
        return explicit.to_string();
    }

    // Spec C2: Use MD5 of service YAML (fallback to image ref)
    let input = serde_yaml::to_string(svc).unwrap_or_else(|_| svc.image_ref(service_name));
    generate_name(&input)
}

impl ComposeService {
    /// Check if the service's container exists.
    pub async fn exists(&self, backend: &dyn ContainerBackend, service_name: &str) -> Result<bool> {
        let name = service_container_name(self, service_name);
        match backend.inspect(&name).await {
            Ok(_) => Ok(true),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Check if the service's container is running.
    pub async fn is_running(&self, backend: &dyn ContainerBackend, service_name: &str) -> Result<bool> {
        let name = service_container_name(self, service_name);
        match backend.inspect(&name).await {
            Ok(info) => Ok(info.status == "running"),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Run the command to create and start the service container.
    pub async fn run_command(&self, backend: &dyn ContainerBackend, service_name: &str) -> Result<()> {
        let name = service_container_name(self, service_name);
        let spec = self.to_container_spec(service_name, Some(&name));
        backend.run(&spec).await.map(|_| ())
    }

    /// Start the existing stopped service container.
    pub async fn start_command(&self, backend: &dyn ContainerBackend, service_name: &str) -> Result<()> {
        let name = service_container_name(self, service_name);
        backend.start(&name).await
    }

    /// Build the image for the service if a build config is provided.
    pub async fn build_command(&self, backend: &dyn ContainerBackend, service_name: &str) -> Result<()> {
        if let Some(build) = &self.build {
            let image_name = self.image_ref(service_name);
            backend.build(&build.as_build(), &image_name).await
        } else {
            Ok(())
        }
    }

    /// Create a ContainerSpec from this service definition.
    pub fn to_container_spec(&self, service_name: &str, container_name: Option<&str>) -> ContainerSpec {
        ContainerSpec {
            image: self.image_ref(service_name),
            name: container_name.map(String::from),
            ports: Some(self.port_strings()),
            volumes: Some(self.volume_strings()),
            env: Some(self.resolved_env()),
            cmd: self.command_list(),
            entrypoint: self.entrypoint.as_ref().map(|e| match e {
                serde_yaml::Value::String(s) => vec![s.clone()],
                serde_yaml::Value::Sequence(seq) => seq.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
                _ => vec![],
            }),
            network: self.network_mode.clone(),
            rm: Some(false),
            read_only: self.read_only,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_name_format() {
        let name = generate_name("nginx:latest");
        // Format: {md5_8chars}-{random_hex8}
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 8);
    }

    #[test]
    fn test_explicit_name() {
        let mut svc = ComposeService::default();
        svc.container_name = Some("my-container".to_string());
        let name = service_container_name(&svc, "web");
        assert_eq!(name, "my-container");
    }
}
