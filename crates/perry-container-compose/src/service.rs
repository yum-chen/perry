use std::collections::HashMap;
use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::{ContainerInfo, ContainerSpec, ListOrDict};
use md5::{Digest, Md5};

/// Ported from internal/entities/service.go
pub struct Service {
    pub image: Option<String>,
    pub name: Option<String>,           // container_name in YAML
    pub ports: Option<Vec<String>>,
    pub environment: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<ServiceBuild>,

    // Additional fields for full compose-spec support (Requirement 19.3)
    pub command: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub env_file: Option<Vec<String>>,
    pub networks: Option<Vec<String>>,
    pub depends_on: Option<crate::types::DependsOnSpec>,
    pub restart: Option<String>,
    pub healthcheck: Option<crate::types::ComposeHealthcheck>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub hostname: Option<String>,
    pub privileged: Option<bool>,
    pub read_only: Option<bool>,
    pub stdin_open: Option<bool>,
    pub tty: Option<bool>,
    pub isolation_level: Option<crate::types::IsolationLevel>,
}

/// Ported from internal/entities/service.go
pub struct ServiceBuild {
    pub context: String,
    pub dockerfile: Option<String>,
    pub args: Option<HashMap<String, String>>,
    pub labels: Option<ListOrDict>,
    pub target: Option<String>,
    pub network: Option<String>,
}

impl Service {
    /// Ported from internal/entities/service.go (Requirement 19.2, 19.4)
    pub fn generate_name(image: &str, service_name: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let short_hash = &hash[..8];

        let random_suffix: u32 = rand::random();

        format!("{}_{}_{}", service_name, short_hash, random_suffix)
    }

    pub fn container_name(&self, service_name: &str) -> String {
        if let Some(name) = self.name.as_ref() {
            return name.clone();
        }
        let image = self.image.as_deref().unwrap_or("unknown");
        Self::generate_name(image, service_name)
    }

    /// Requirement 19.2: Ported from internal/entities/service.go
    pub async fn exists(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.container_name(service_name)).await {
            Ok(_) => Ok(true),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Requirement 19.2: Ported from internal/entities/service.go
    pub async fn is_running(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.container_name(service_name)).await {
            Ok(info) => Ok(info.status.to_lowercase().contains("running")
                || info.status.to_lowercase().contains("up")),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Requirement 19.2: Ported from internal/entities/service.go
    pub fn needs_build(&self) -> bool {
        self.build.is_some() && self.image.is_none()
    }

    /// Requirement 19.2: Ported from internal/entities/service.go
    pub async fn run_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        if self.needs_build() {
            self.build_command(service_name, backend).await?;
        }

        let spec = ContainerSpec {
            image: self.image.clone().unwrap_or_else(|| format!("{}_image", service_name)),
            name: Some(self.container_name(service_name)),
            ports: self.ports.clone(),
            volumes: self.volumes.clone(),
            env: self.environment.as_ref().map(|e| e.to_map()),
            cmd: self.command.clone(),
            entrypoint: self.entrypoint.clone(),
            network: self.networks.as_ref().and_then(|n| n.first().cloned()),
            rm: Some(false),
        };

        backend.run(&spec).await?;
        Ok(())
    }

    /// Requirement 19.2: Ported from internal/entities/service.go
    pub async fn start_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        backend.start(&self.container_name(service_name)).await
    }

    /// Requirement 19.2: Ported from internal/entities/service.go
    pub async fn build_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build) = &self.build {
            // Need a way to convert ServiceBuild to whatever backend.build takes
            // Assuming we'll refactor backend.build too.
            let image_name = self.image.clone().unwrap_or_else(|| format!("{}_image", service_name));

            // Temporary mapping until backend is refactored
            let build_spec = crate::types::ComposeServiceBuild {
                context: Some(build.context.clone()),
                dockerfile: build.dockerfile.clone(),
                args: build.args.as_ref().map(|a| {
                    let mut map = indexmap::IndexMap::new();
                    for (k, v) in a {
                        map.insert(k.clone(), Some(serde_yaml::Value::String(v.clone())));
                    }
                    crate::types::ListOrDict::Dict(map)
                }),
                labels: build.labels.clone(),
                target: build.target.clone(),
                network: build.network.clone(),
                ..Default::default()
            };

            backend.build(&build_spec, &image_name).await?;
        }
        Ok(())
    }

    /// Requirement 19.2: Ported from internal/entities/service.go
    pub async fn inspect_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
        backend.inspect(&self.container_name(service_name)).await
    }
}
