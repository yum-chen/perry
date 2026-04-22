use crate::error::{ComposeError, Result};
use crate::backend::ContainerBackend;
use crate::types::{ComposeService, ContainerInfo, ContainerSpec};
use md5::{Digest, Md5};

pub fn service_container_name(service: &ComposeService, service_name: &str) -> String {
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

pub struct Service {
    pub name: String,
    pub config: ComposeService,
}

impl Service {
    pub fn new(name: String, config: ComposeService) -> Self {
        Self { name, config }
    }

    pub fn container_name(&self) -> String {
        if let Some(name) = self.config.container_name.as_ref() {
            return name.clone();
        }

        let image = self.config.image.as_deref().unwrap_or("unknown");
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let short_hash = &hash[..8];

        let random_suffix: u32 = rand::random();

        let safe_name: String = self.name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();

        format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
    }

    pub async fn exists(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.container_name()).await {
            Ok(_) => Ok(true),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.container_name()).await {
            Ok(info) => Ok(info.status == "running"),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn needs_build(&self) -> bool {
        self.config.build.is_some() && self.config.image.is_none()
    }

    pub async fn run_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        if self.needs_build() {
            self.build_command(backend).await?;
        }

        let spec = ContainerSpec {
            image: self.config.image_ref(&self.name),
            name: Some(self.container_name()),
            ports: Some(self.config.port_strings()),
            volumes: Some(self.config.volume_strings()),
            env: Some(self.config.resolved_env()),
            cmd: self.config.command_list(),
            rm: Some(false),
            ..Default::default()
        };

        backend.run(&spec).await.map(|_| ())
    }

    pub async fn start_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.start(&self.container_name()).await
    }

    pub async fn build_command(&self, _backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(_build) = &self.config.build {
            // let build_config = _build.as_build();
            // _backend.build(&build_config, &self.config.image_ref(&self.name)).await
            Ok(())
        } else {
            Ok(())
        }
    }

    pub async fn inspect_command(&self, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
        backend.inspect(&self.container_name()).await
    }
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
