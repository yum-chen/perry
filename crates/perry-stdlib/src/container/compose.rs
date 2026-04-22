use perry_container_compose::ComposeEngine;
use perry_container_compose::types::ComposeSpec;
use crate::container::backend::ContainerBackend;
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Result<ComposeWrapper, perry_container_compose::error::ComposeError> {
    let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
    let engine = ComposeEngine::new(spec, project_name, backend);
    Ok(ComposeWrapper {
        engine: Arc::new(engine),
    })
}
