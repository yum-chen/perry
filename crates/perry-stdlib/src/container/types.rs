//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::context::ContainerContext;
use perry_container_compose::ComposeEngine;
use std::sync::Arc;

// ============ Handle Registry ============

pub fn register_container_handle(h: ContainerHandle) -> u64 {
    ContainerContext::global().register_handle(h)
}

pub fn register_compose_engine(engine: Arc<ComposeEngine>, _stack_id: u64) -> u64 {
    ContainerContext::global().register_handle(engine)
}

pub fn get_compose_engine(id: u64) -> Option<Arc<ComposeEngine>> {
    ContainerContext::global().get_handle::<ComposeEngine>(id)
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    ContainerContext::global().register_handle(list)
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    ContainerContext::global().register_handle(info)
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    ContainerContext::global().register_handle(logs)
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    ContainerContext::global().register_handle(list)
}

pub fn register_string(s: String) -> u64 {
    ContainerContext::global().register_handle(s)
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    ContainerContext::global().handles.remove(&id).and_then(|(_, v)| v.downcast::<Vec<ContainerInfo>>().ok()).map(|b| *b)
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    ContainerContext::global().handles.remove(&id).and_then(|(_, v)| v.downcast::<ContainerLogs>().ok()).map(|b| *b)
}

// ============ Core Container Types ============

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
}

impl From<ContainerSpec> for perry_container_compose::types::ContainerSpec {
    fn from(spec: ContainerSpec) -> Self {
        Self {
            image: spec.image,
            name: spec.name,
            ports: spec.ports,
            volumes: spec.volumes,
            env: spec.env,
            cmd: spec.cmd,
            entrypoint: spec.entrypoint,
            network: spec.network,
            rm: spec.rm,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

impl From<perry_container_compose::types::ContainerHandle> for ContainerHandle {
    fn from(h: perry_container_compose::types::ContainerHandle) -> Self {
        Self { id: h.id, name: h.name }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
}

impl From<perry_container_compose::types::ContainerInfo> for ContainerInfo {
    fn from(info: perry_container_compose::types::ContainerInfo) -> Self {
        Self {
            id: info.id,
            name: info.name,
            image: info.image,
            status: info.status,
            ports: info.ports,
            created: info.created,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

impl From<perry_container_compose::types::ContainerLogs> for ContainerLogs {
    fn from(logs: perry_container_compose::types::ContainerLogs) -> Self {
        Self {
            stdout: logs.stdout,
            stderr: logs.stderr,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}

impl From<perry_container_compose::types::ImageInfo> for ImageInfo {
    fn from(info: perry_container_compose::types::ImageInfo) -> Self {
        Self {
            id: info.id,
            repository: info.repository,
            tag: info.tag,
            size: info.size,
            created: info.created,
        }
    }
}

pub unsafe fn parse_container_spec_json(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub unsafe fn parse_compose_spec_json(ptr: *const StringHeader) -> Result<perry_container_compose::types::ComposeSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}
