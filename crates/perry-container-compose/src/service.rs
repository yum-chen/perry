use crate::error::{Result, ComposeError};
use crate::backend::ContainerBackend;
use crate::types::ComposeService;
use md5::{Digest, Md5};

impl ComposeService {
    pub async fn exists(&self, service_name: &str, backend: &dyn ContainerBackend) -> bool {
        let name = service_container_name(self, service_name);
        backend.inspect(&name).await.is_ok()
    }

    pub async fn is_running(&self, service_name: &str, backend: &dyn ContainerBackend) -> bool {
        let name = service_container_name(self, service_name);
        match backend.inspect(&name).await {
            Ok(info) => info.status == "running",
            Err(_) => false,
        }
    }

    pub async fn build_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build_spec) = &self.build {
            let build_config = build_spec.as_build();
            let tag = self.image_ref(service_name);
            backend.build(&build_config, &tag).await
        } else {
            Ok(())
        }
    }
}

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
