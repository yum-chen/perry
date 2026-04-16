//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};

use crate::common::handle::{self, Handle};

// Re-export core types from the compose crate to avoid duplication and mismatch.
pub use perry_container_compose::types::{
    ComposeHandle, ComposeSpec, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo, ListOrDict,
};

// ============ Handle Registry ============

pub fn register_container_handle(h: ContainerHandle) -> u64 {
    handle::register_handle(h) as u64
}

pub fn get_container_handle(id: u64) -> Option<Handle> {
    let h = id as Handle;
    if handle::handle_exists(h) {
        Some(h)
    } else {
        None
    }
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn with_container_info_list<R>(id: u64, f: impl FnOnce(&Vec<ContainerInfo>) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    handle::take_handle(id as Handle)
}

pub fn register_compose_handle(h: ComposeHandle) -> u64 {
    handle::register_handle(h) as u64
}

pub fn get_compose_handle(id: u64) -> Option<&'static ComposeHandle> {
    handle::get_handle(id as Handle)
}

pub fn take_compose_handle(id: u64) -> Option<ComposeHandle> {
    handle::take_handle(id as Handle)
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

pub fn with_container_logs<R>(id: u64, f: impl FnOnce(&ContainerLogs) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    handle::take_handle(id as Handle)
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn with_image_info_list<R>(id: u64, f: impl FnOnce(&Vec<ImageInfo>) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

pub fn take_image_info_list(id: u64) -> Option<Vec<ImageInfo>> {
    handle::take_handle(id as Handle)
}

pub fn drop_container_handle(id: u64) -> bool {
    handle::drop_handle(id as Handle)
}

// ============ Error Types ============

/// Container module errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerError {
    NotFound(String),
    BackendError {
        code: i32,
        message: String,
    },
    VerificationFailed {
        image: String,
        reason: String,
    },
    DependencyCycle {
        services: Vec<String>,
    },
    ServiceStartupFailed {
        service: String,
        error: String,
    },
    InvalidConfig(String),
    NoBackendFound {
        probed: Vec<crate::container::backend::BackendProbeResult>,
    },
    BackendNotAvailable {
        name: String,
        reason: String,
    },
}

impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::NotFound(id) => write!(f, "Container not found: {}", id),
            ContainerError::BackendError { code, message } => {
                write!(f, "Backend error (code {}): {}", code, message)
            }
            ContainerError::VerificationFailed { image, reason } => {
                write!(f, "Image verification failed for {}: {}", image, reason)
            }
            ContainerError::DependencyCycle { services } => {
                write!(f, "Dependency cycle detected: {}", services.join(" -> "))
            }
            ContainerError::ServiceStartupFailed { service, error } => {
                write!(f, "Service {} failed to start: {}", service, error)
            }
            ContainerError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            ContainerError::NoBackendFound { probed } => {
                write!(f, "No container backend found. Probed: {:?}", probed)
            }
            ContainerError::BackendNotAvailable { name, reason } => {
                write!(f, "Backend '{}' not available: {}", name, reason)
            }
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<perry_container_compose::error::ComposeError> for ContainerError {
    fn from(e: perry_container_compose::error::ComposeError) -> Self {
        match e {
            perry_container_compose::error::ComposeError::NotFound(s) => ContainerError::NotFound(s),
            perry_container_compose::error::ComposeError::BackendError { code, message } => {
                ContainerError::BackendError { code, message }
            }
            perry_container_compose::error::ComposeError::VerificationFailed { image, reason } => {
                ContainerError::VerificationFailed { image, reason }
            }
            perry_container_compose::error::ComposeError::DependencyCycle { services } => {
                ContainerError::DependencyCycle { services }
            }
            perry_container_compose::error::ComposeError::ServiceStartupFailed {
                service,
                message,
            } => ContainerError::ServiceStartupFailed {
                service,
                error: message,
            },
            perry_container_compose::error::ComposeError::ValidationError { message } => {
                ContainerError::InvalidConfig(message)
            }
            perry_container_compose::error::ComposeError::NoBackendFound { probed } => {
                ContainerError::NoBackendFound { probed }
            }
            perry_container_compose::error::ComposeError::BackendNotAvailable { name, reason } => {
                ContainerError::BackendNotAvailable { name, reason }
            }
            perry_container_compose::error::ComposeError::ParseError(e) => {
                ContainerError::InvalidConfig(e.to_string())
            }
            perry_container_compose::error::ComposeError::JsonError(e) => {
                ContainerError::InvalidConfig(e.to_string())
            }
            perry_container_compose::error::ComposeError::IoError(e) => {
                ContainerError::BackendError {
                    code: -1,
                    message: e.to_string(),
                }
            }
            perry_container_compose::error::ComposeError::FileNotFound { path } => {
                ContainerError::NotFound(format!("File not found: {}", path))
            }
        }
    }
}

// ============ JSON Parsing ============

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Parse `ContainerSpec` from a JSON StringHeader pointer.
pub fn parse_container_spec(spec_ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

/// Parse `ComposeSpec` from a JSON StringHeader pointer.
pub fn parse_compose_spec(spec_ptr: *const StringHeader) -> Result<ComposeSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}
