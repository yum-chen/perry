use std::sync::{Arc, OnceLock};
use perry_container_compose::backend::ContainerBackend;
use tokio::sync::Mutex;

/// Scoped state container for container operations.
pub struct ContainerContext {
    pub backend: Mutex<Option<Arc<dyn ContainerBackend>>>,
}

impl ContainerContext {
    /// Returns the process-global default context.
    pub fn global() -> &'static ContainerContext {
        static GLOBAL: OnceLock<ContainerContext> = OnceLock::new();
        GLOBAL.get_or_init(|| ContainerContext {
            backend: Mutex::new(None),
        })
    }

    /// Creates a new isolated context (for tests or multi-tenant use).
    pub fn new() -> Self {
        ContainerContext {
            backend: Mutex::new(None),
        }
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, String> {
        let mut lock = self.backend.lock().await;
        if let Some(b) = &*lock {
            return Ok(Arc::clone(b));
        }

        let backend = perry_container_compose::backend::detect_backend().await
            .map(Arc::from)
            .map_err(|probed| format!("No container backend found: {:?}", probed))?;

        *lock = Some(Arc::clone(&backend));
        Ok(backend)
    }
}
