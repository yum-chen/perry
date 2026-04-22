//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use perry_container_compose::backend::ContainerBackend;
use super::types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerInfo, ContainerLogs,
};
use std::sync::Arc;
use perry_container_compose::ComposeEngine;

pub struct ComposeWrapper {
    engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    /// Create a new `ComposeWrapper` from a spec and backend.
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    /// Create a `ComposeWrapper` from an already-existing engine (e.g. looked up by stack ID).
    pub fn from_engine(engine: Arc<ComposeEngine>) -> Self {
        Self { engine }
    }

    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        Arc::clone(&self.engine).up(&[], true, false, false).await.map_err(ContainerError::from)
    }

    pub async fn down(&self, volumes: bool) -> Result<(), ContainerError> {
        self.engine.down(&[], false, volumes).await.map_err(ContainerError::from)
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>, ContainerError> {
        let infos = self.engine.ps().await.map_err(ContainerError::from)?;
        Ok(infos.into_iter().map(Into::into).collect())
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        let services = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let logs = self.engine.logs(&services, tail).await.map_err(ContainerError::from)?;
        Ok(logs)
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs, ContainerError> {
        let res = self.engine.exec(service, cmd, None, None).await.map_err(ContainerError::from)?;
        Ok(res.into())
    }

    pub async fn start(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.start(services).await.map_err(ContainerError::from)
    }

    pub async fn stop(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.stop(services).await.map_err(ContainerError::from)
    }

    pub async fn restart(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.restart(services).await.map_err(ContainerError::from)
    }
}

/// Create a new compose stack from a spec and backend, bring it up, and return the handle.
pub async fn compose_up(
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<ComposeHandle, ContainerError> {
    let wrapper = ComposeWrapper::new(spec, backend);
    wrapper.up().await
}
