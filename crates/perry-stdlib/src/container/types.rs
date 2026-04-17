//! Type definitions for the perry/container module.

pub use perry_container_compose::types::{
    ComposeConfig, ComposeDeployment, ComposeDependsOn, ComposeHandle,
    ComposeHealthcheck, ComposeLogging, ComposeNetwork,
    ComposeNetworkIpam, ComposeNetworkIpamConfig, ComposeSecret,
    ComposeService, ComposeServiceNetworkConfig, ComposeServicePort,
    ComposeServiceVolume, ComposeSpec, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

pub use perry_container_compose::ComposeError;

use perry_runtime::StringHeader;
use crate::common::handle::{self, Handle};

// ============ Handle Registry ============

pub fn register_container_handle(h: ContainerHandle) -> u64 {
    handle::register_handle(h) as u64
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn register_compose_handle(h: ComposeHandle) -> u64 {
    handle::register_handle(h) as u64
}

pub fn get_compose_handle(id: u64) -> Option<ComposeHandle> {
    handle::get_handle::<ComposeHandle>(id as Handle).cloned()
}

pub fn register_string(s: String) -> u64 {
    handle::register_handle(s) as u64
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Backend error (code {code}): {message}")]
    BackendError { code: i32, message: String },
    #[error("Verification failed for {image}: {reason}")]
    VerificationFailed { image: String, reason: String },
    #[error("No backend found: {probed:?}")]
    NoBackendFound { probed: Vec<perry_container_compose::BackendProbeResult> },
    #[error("Backend not available: {name} ({reason})")]
    BackendNotAvailable { name: String, reason: String },
    #[error("Compose error: {0}")]
    Compose(#[from] perry_container_compose::ComposeError),
}

// ============ JSON Parsing ============

pub unsafe fn parse_container_spec_json(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub unsafe fn parse_compose_spec_json(ptr: *const StringHeader) -> Result<ComposeSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}
