//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use dashmap::DashMap;

use perry_container_compose::ComposeEngine;

// ============ Handle Registry ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ArcComposeEngine>> = OnceLock::new();
pub static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub struct ArcComposeEngine(pub std::sync::Arc<ComposeEngine>);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_compose_handle(engine: ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, ArcComposeEngine(std::sync::Arc::new(engine)));
    id
}

pub use perry_container_compose::types::{
    BackendInfo, ComposeHandle, ComposeSpec, ContainerHandle, ContainerInfo, ContainerLogs,
    ContainerSpec, ImageInfo, IsolationLevel, ServiceGraph, StackStatus,
};

// ============ Helper for StringHeader ============

pub unsafe fn string_from_header(header: *const StringHeader) -> Option<String> {
    if header.is_null() || (header as usize) < 0x1000 {
        return None;
    }
    let blen = (*header).byte_len as usize;
    let data = (header as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data, blen);
    Some(String::from_utf8_lossy(bytes).into_owned())
}
