use crate::container::types::{ComposeHandle, ContainerError};
use perry_container_compose::backend::{ContainerBackend};
use perry_container_compose::types::{ComposeSpec};
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeWrapper {
    pub engine: Arc<perry_container_compose::compose::ComposeEngine>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, _project_name: String, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self {
            engine: Arc::new(perry_container_compose::compose::ComposeEngine::new(spec, backend)),
        }
    }
}

pub async fn compose_up(spec: ComposeSpec) -> Result<ComposeHandle, ContainerError> {
    let backend = crate::container::backend::get_global_backend_instance().await?;
    let engine = perry_container_compose::compose::ComposeEngine::new(spec, backend);
    let handle = engine.up().await.map_err(|e| ContainerError::OperationFailed(e.to_string()))?;

    Ok(ComposeHandle {
        stack_id: handle.stack_id,
        project_name: handle.project_name,
        services: handle.services,
    })
}
