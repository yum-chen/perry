pub use perry_container_compose::error::{BackendProbeResult, ComposeError};
use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHandle { pub id: String, pub name: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String, pub name: String, pub image: String,
    pub status: String, pub ports: Vec<String>, pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogs { pub stdout: String, pub stderr: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String, pub repository: String, pub tag: String,
    pub size: u64, pub created: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<BackendProbeResult> },

    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),

    #[error("Container operation failed: {0}")]
    OperationFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Image verification failed for '{image}': {reason}")]
    VerificationFailed { image: String, reason: String },
}

impl From<ComposeError> for ContainerError {
    fn from(e: ComposeError) -> Self {
        match e {
            ComposeError::NoBackendFound { probed } => ContainerError::NoBackendFound { probed },
            ComposeError::BackendNotAvailable { name, .. } => ContainerError::BackendNotAvailable(name),
            _ => ContainerError::OperationFailed(e.to_string()),
        }
    }
}

pub unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() { return None; }
    let header = &*ptr;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, header.byte_len as usize);
    Some(String::from_utf8_lossy(bytes).to_string())
}

pub use perry_container_compose::types::{
    ComposeHealthcheck, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume, ListOrDict
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn register_container_handle(_handle: ContainerHandle) -> u64 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_info(_info: ContainerInfo) -> u64 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_info_list(_list: Vec<ContainerInfo>) -> u64 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_compose_handle(_handle: ComposeHandle) -> u64 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_logs(_logs: ContainerLogs) -> u64 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_image_info_list(_list: Vec<ImageInfo>) -> u64 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}
