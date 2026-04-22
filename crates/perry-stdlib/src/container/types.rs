//! Type re-exports for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;

use perry_runtime::JSValue;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use dashmap::DashMap;

// ============ Handle Management ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_container_info(_info: ContainerInfo) -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_info_list(_list: Vec<ContainerInfo>) -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_compose_handle(_handle: ComposeHandle) -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_logs(_logs: ContainerLogs) -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_image_info_list(_list: Vec<ImageInfo>) -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

// ============ JSValue Parsing Functions ============

pub fn parse_container_spec(_spec_ptr: *const JSValue) -> Result<ContainerSpec, String> {
    Err("ContainerSpec parsing must be done at compile time.".to_string())
}

pub fn parse_compose_spec(_spec_ptr: *const JSValue) -> Result<ComposeSpec, String> {
    Err("ComposeSpec parsing must be done at compile time.".to_string())
}
