//! Container context for Perry.
//!
//! Owns the backend instance and the handle registry.

use crate::container::backend::{detect_backend, ContainerBackend};
use crate::container::types::ContainerError;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::sync::OnceLock;

pub enum HandleEntry {
    Container(perry_container_compose::types::ContainerHandle),
    Compose(perry_container_compose::types::ComposeHandle),
    Engine(Arc<perry_container_compose::ComposeEngine>),
    InfoList(Vec<perry_container_compose::types::ContainerInfo>),
    ImageList(Vec<perry_container_compose::types::ImageInfo>),
    Logs(perry_container_compose::types::ContainerLogs),
}

pub struct ContainerContext {
    backend: Mutex<OnceLock<Arc<dyn ContainerBackend>>>,
    pub handles: DashMap<u64, HandleEntry>,
}

impl ContainerContext {
    pub fn new() -> Self {
        Self {
            backend: Mutex::new(OnceLock::new()),
            handles: DashMap::new(),
        }
    }

    pub fn global() -> &'static ContainerContext {
        static INSTANCE: OnceLock<ContainerContext> = OnceLock::new();
        INSTANCE.get_or_init(ContainerContext::new)
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, ContainerError> {
        let lock = self.backend.lock().await;
        if let Some(b) = lock.get() {
            return Ok(Arc::clone(b));
        }

        let b = detect_backend()
            .await
            .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
            .map_err(ContainerError::from)?;

        let _ = lock.set(Arc::clone(&b));
        Ok(b)
    }

    pub fn register_handle(&self, entry: HandleEntry) -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        self.handles.insert(id, entry);
        id
    }
}
