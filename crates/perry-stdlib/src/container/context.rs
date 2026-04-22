//! `ContainerContext` — scoped state container for the container module.

use crate::container::backend::ContainerBackend;
use crate::common::handle::{Handle};
use dashmap::DashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

/// Scoped state container for backends and handles.
pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend>>,
    pub backend_mutex: Mutex<()>,
    pub handles: DashMap<u64, HandleEntry>,
}

/// Entry in the handle registry.
pub enum HandleEntry {
    Container(perry_container_compose::types::ContainerHandle),
    Compose(perry_container_compose::types::ComposeHandle),
    Graph(crate::container::workload::GraphHandle),
    Info(perry_container_compose::types::ContainerInfo),
    InfoList(Vec<perry_container_compose::types::ContainerInfo>),
    Logs(perry_container_compose::types::ContainerLogs),
    ImageList(Vec<perry_container_compose::types::ImageInfo>),
}

static GLOBAL_CONTEXT: OnceLock<ContainerContext> = OnceLock::new();

impl ContainerContext {
    /// Returns the process-global default context.
    pub fn global() -> &'static ContainerContext {
        GLOBAL_CONTEXT.get_or_init(Self::new)
    }

    /// Creates a new isolated context.
    pub fn new() -> Self {
        Self {
            backend: OnceLock::new(),
            backend_mutex: Mutex::new(()),
            handles: DashMap::new(),
        }
    }

    /// Get or initialize the backend instance for this context.
    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, crate::container::types::ContainerError> {
        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        let _guard = self.backend_mutex.lock().await;
        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        let b = crate::container::backend::detect_backend().await
            .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
            .map_err(crate::container::types::ContainerError::from)?;

        let _ = self.backend.set(Arc::clone(&b));
        Ok(b)
    }

    pub fn register_handle(&self, entry: HandleEntry) -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        self.handles.insert(id, entry);
        id
    }

    pub fn get_handle(&self, id: u64) -> Option<dashmap::mapref::one::Ref<u64, HandleEntry>> {
        self.handles.get(&id)
    }

    pub fn take_handle(&self, id: u64) -> Option<HandleEntry> {
        self.handles.remove(&id).map(|(_, v)| v)
    }
}
