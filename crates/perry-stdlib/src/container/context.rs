use std::sync::{Arc, OnceLock};
use perry_container_compose::backend::{detect_backend, ContainerBackend};
use dashmap::DashMap;
use std::any::Any;
use tokio::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ContainerContext {
    /// Requirement 4.6: tokio::sync::Mutex for async double-checked init
    backend: Mutex<Option<Arc<dyn ContainerBackend>>>,
    /// Task 12: DashMap handle registry
    pub handles: DashMap<u64, Box<dyn Any + Send + Sync>>,
    next_handle_id: AtomicU64,
}

static GLOBAL_CONTEXT: OnceLock<ContainerContext> = OnceLock::new();

impl ContainerContext {
    pub fn new() -> Self {
        Self {
            backend: Mutex::new(None),
            handles: DashMap::new(),
            next_handle_id: AtomicU64::new(1),
        }
    }

    pub fn global() -> &'static ContainerContext {
        GLOBAL_CONTEXT.get_or_init(Self::new)
    }

    pub fn register_handle<T: 'static + Send + Sync>(&self, value: T) -> u64 {
        let id = self.next_handle_id.fetch_add(1, Ordering::SeqCst);
        self.handles.insert(id, Box::new(value));
        id
    }

    pub fn get_handle<T: 'static + Send + Sync>(&self, id: u64) -> Option<Arc<T>> {
        self.handles.get(&id).and_then(|entry| {
            entry.value().downcast_ref::<Arc<T>>().map(|arc| Arc::clone(arc))
        })
    }

    pub fn try_get_backend(&self) -> Option<Arc<dyn ContainerBackend>> {
        self.backend.blocking_lock().clone()
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, String> {
        let mut lock = self.backend.lock().await;
        if let Some(b) = lock.as_ref() {
            return Ok(Arc::clone(b));
        }

        match detect_backend().await {
            Ok(b) => {
                let shared = Arc::from(b);
                *lock = Some(Arc::clone(&shared));
                Ok(shared)
            }
            Err(e) => {
                // Requirement 20: Installer invocation
                let installer = perry_container_compose::installer::BackendInstaller::new();
                match installer.run().await {
                    Ok(driver) => {
                        let backend = Arc::from(driver.into_backend());
                        *lock = Some(Arc::clone(&backend));
                        Ok(backend)
                    }
                    Err(_) => Err(format!("No backend found: {:?}", e)),
                }
            }
        }
    }
}
