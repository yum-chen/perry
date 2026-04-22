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
        self.engine.up(&[], true, false, false).await.map_err(Into::into)
    }

    pub async fn down(&self, _handle: &ComposeHandle, volumes: bool) -> Result<(), ContainerError> {
        self.engine.down(&[], false, volumes).await.map_err(Into::into)
    }

    pub async fn ps(&self, _handle: &ComposeHandle) -> Result<Vec<ContainerInfo>, ContainerError> {
        self.engine.ps().await.map_err(Into::into)
    }

    pub async fn logs(&self, _handle: &ComposeHandle, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        self.engine.logs(service, tail).await.map_err(Into::into)
    }

    pub async fn exec(&self, _handle: &ComposeHandle, service: &str, cmd: &[String]) -> Result<ContainerLogs, ContainerError> {
        self.engine.exec(service, cmd).await.map_err(Into::into)
    }

    pub async fn start(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.start(services).await.map_err(Into::into)
    }

    pub async fn stop(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.stop(services).await.map_err(Into::into)
    }

    pub async fn restart(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.restart(services).await.map_err(Into::into)
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

/// Look up an existing engine by stack ID and wrap it in a `ComposeWrapper`.
/// Returns `None` if no engine with that stack ID is registered.
pub fn get_engine_wrapper(stack_id: u64) -> Option<ComposeWrapper> {
    ComposeEngine::get_engine(stack_id).map(ComposeWrapper::from_engine)
}
