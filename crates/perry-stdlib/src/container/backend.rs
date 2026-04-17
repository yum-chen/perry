//! Backend re-exports and global instance management.

use super::types::ContainerError;
pub use perry_container_compose::backend::{
    detect_backend, AppleBackend, AppleContainerProtocol, BackendProbeResult, CliBackend,
    CliProtocol, ContainerBackend, DockerBackend, DockerProtocol, LimaBackend, LimaProtocol,
    NetworkConfig, VolumeConfig,
};
use std::sync::{Arc, OnceLock};

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

/// Get the global container backend instance, initializing it if necessary.
pub async fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let backend = detect_backend()
        .await
        .map(Arc::from)
        .map_err(|probed| ContainerError::NoBackendFound { probed })?;

    let _ = BACKEND.set(Arc::clone(&backend));
    Ok(backend)
}

/// Helper to map compose errors to stdlib container errors.
pub fn map_compose_err(e: perry_container_compose::ComposeError) -> ContainerError {
    ContainerError::Compose(e)
}
