use perry_container_compose::backend::ContainerBackend;
use std::sync::{Arc, OnceLock};
use crate::container::types::ContainerError;

static BACKEND: OnceLock<Arc<dyn ContainerBackend + Send + Sync>> = OnceLock::new();

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, ContainerError> {
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let backend_arc = perry_container_compose::backend::detect_backend().await
        .map_err(|e| ContainerError::NoBackendFound { probed: e })?;

    let _ = BACKEND.set(Arc::clone(&backend_arc));
    Ok(backend_arc)
}
