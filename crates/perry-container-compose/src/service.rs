use crate::error::Result;
use md5::{Digest, Md5};

use crate::backend::ContainerBackend;
use crate::types::{ContainerInfo, ContainerSpec};

pub fn service_container_name(service: &crate::types::ComposeService, service_name: &str) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }

    let image = service.image.as_deref().unwrap_or("unknown");
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];

    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}

pub struct Service<'a> {
    pub name: String,
    pub spec: &'a crate::types::ComposeService,
}

impl<'a> Service<'a> {
    pub fn new(name: String, spec: &'a crate::types::ComposeService) -> Self {
        Self { name, spec }
    }

    pub fn container_name(&self) -> String {
        service_container_name(self.spec, &self.name)
    }

    pub async fn exists(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.container_name()).await {
            Ok(_) => Ok(true),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.container_name()).await {
            Ok(info) => Ok(info.status.to_lowercase().contains("running")
                || info.status.to_lowercase().contains("up")),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn needs_build(&self) -> bool {
        self.spec.build.is_some() && self.spec.image.is_none()
    }

    pub async fn run_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let network = match &self.spec.networks {
            Some(crate::types::ServiceNetworks::List(l)) => l.first().cloned(),
            Some(crate::types::ServiceNetworks::Map(m)) => m.keys().next().cloned(),
            None => None,
        };

        let container_spec = ContainerSpec {
            image: self.spec.image.clone().unwrap_or_else(|| format!("{}-image", self.name)),
            name: Some(self.container_name()),
            ports: Some(self.spec.port_strings()),
            volumes: Some(self.spec.volume_strings()),
            env: Some(self.spec.resolved_env()),
            cmd: self.spec.command_list(),
            entrypoint: None,
            network,
            rm: None,
            read_only: self.spec.read_only,
            seccomp: None,
            isolation_level: self.spec.isolation_level.clone(),
        };

        backend.run(&container_spec).await?;
        Ok(())
    }

    pub async fn start_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.start(&self.container_name()).await
    }

    pub async fn build_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build) = &self.spec.build {
            backend.build(&build.as_build(), &format!("{}-image", self.name)).await?;
        }
        Ok(())
    }

    pub async fn inspect_command(&self, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
        backend.inspect(&self.container_name()).await
    }
}
