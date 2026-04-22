//! Type re-exports for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::{ComposeError, BackendProbeResult};

use perry_runtime::JSValue;
use std::sync::atomic::{AtomicU64, Ordering};

// ============ Handle Management ============

static NEXT_CONTAINER_HANDLE: AtomicU64 = AtomicU64::new(1);

pub fn register_container_handle(_handle: ContainerHandle) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_info(_info: ContainerInfo) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_info_list(_list: Vec<ContainerInfo>) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

pub fn register_compose_handle(_handle: ComposeHandle) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_logs(_logs: ContainerLogs) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

pub fn register_image_info_list(_list: Vec<ImageInfo>) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

pub fn register_json_value(_val: serde_json::Value) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

// ============ JSValue Parsing Functions ============

pub fn parse_container_spec(_spec_ptr: *const JSValue) -> Result<ContainerSpec, String> {
    Err("ContainerSpec parsing must be done at compile time.".to_string())
}

pub fn parse_compose_spec(_spec_ptr: *const JSValue) -> Result<ComposeSpec, String> {
    Err("ComposeSpec parsing must be done at compile time.".to_string())
}
