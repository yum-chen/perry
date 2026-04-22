//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use super::types::{ContainerInfo, ContainerLogs};
use perry_container_compose::ComposeEngine;
use perry_container_compose::types::ComposeSpec;
use std::sync::Arc;
use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::error::ComposeError;

pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    pub fn from_engine(engine: Arc<ComposeEngine>) -> Self {
        Self { engine }
    }

    pub async fn up(self: Arc<Self>) -> Result<Arc<ComposeEngine>, ComposeError> {
        self.engine.up(&[], true, false, false).await?;
        Ok(Arc::clone(&self.engine))
    }
}

pub async fn compose_up(
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<Arc<ComposeEngine>, ComposeError> {
    let wrapper = Arc::new(ComposeWrapper::new(spec, backend));
    wrapper.up().await
}
