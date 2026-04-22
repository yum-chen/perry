use std::sync::{Arc, Mutex, OnceLock};
use perry_container_compose::backend::ContainerBackend;
use crate::common::handle::Handle;

/// Scoped state container for container operations.
pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend>>,
}

impl ContainerContext {
    /// Returns the process-global default context.
    pub fn global() -> &'static ContainerContext {
        static GLOBAL: OnceLock<ContainerContext> = OnceLock::new();
        GLOBAL.get_or_init(|| ContainerContext {
            backend: OnceLock::new(),
        })
    }

    /// Creates a new isolated context (for tests or multi-tenant use).
    pub fn new() -> Self {
        ContainerContext {
            backend: OnceLock::new(),
        }
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, String> {
        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        let backend = perry_container_compose::backend::detect_backend().await
            .map(Arc::from)
            .map_err(|probed| format!("No container backend found: {:?}", probed))?;

        // We ignore the error if it was already set by another thread
        let _ = self.backend.set(Arc::clone(&backend));
        Ok(backend)
    }
}
