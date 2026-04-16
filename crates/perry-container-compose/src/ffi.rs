//! FFI exports for Perry TypeScript integration.
//!
//! Each function follows the Perry FFI convention:
//! - String arguments arrive as `*const StringHeader` (Perry runtime layout)
//! - Results are serialised to JSON strings before being handed back to JS

use crate::compose::ComposeEngine;
use crate::project::ComposeProject;
use crate::types::ComposeHandle;
use crate::backend::detect_backend;
use std::sync::Arc;
use serde_json::json;

// ──────────────────────────────────────────────────────────────
// Minimal re-implementation of the Perry runtime string types
// ──────────────────────────────────────────────────────────────

#[repr(C)]
pub struct StringHeader {
    pub length: u32,
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

fn json_ok(value: &str) -> *const StringHeader {
    let payload = format!("{{\"ok\":true,\"result\":{}}}", value);
    heap_string(payload)
}

fn json_err(message: &str) -> *const StringHeader {
    let escaped = message.replace('"', "\\\"");
    let payload = format!("{{\"ok\":false,\"error\":\"{}\"}}", escaped);
    heap_string(payload)
}

fn heap_string(s: String) -> *const StringHeader {
    let bytes = s.into_bytes();
    let total = std::mem::size_of::<StringHeader>() + bytes.len();
    let layout = std::alloc::Layout::from_size_align(total, std::mem::align_of::<StringHeader>())
        .expect("layout");
    unsafe {
        let ptr = std::alloc::alloc(layout) as *mut StringHeader;
        (*ptr).length = bytes.len() as u32;
        let data_ptr = (ptr as *mut u8).add(std::mem::size_of::<StringHeader>());
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_ptr, bytes.len());
        ptr as *const StringHeader
    }
}

fn block<F: std::future::Future<Output = T>, T>(fut: F) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(fut)
}

// ──────────────────────────────────────────────────────────────
// Exported FFI functions (Internal handle-based mapping)
// ──────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => return json_err("invalid spec JSON"),
    };
    let spec = match serde_json::from_str(&json) {
        Ok(s) => s,
        Err(e) => return json_err(&format!("parse error: {}", e)),
    };

    let backend = match block(detect_backend()) {
        Ok(b) => Arc::from(b),
        Err(_) => return json_err("no container backend found"),
    };

    let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
    let engine = ComposeEngine::new(spec, project_name, backend);

    match block(engine.up(&[], true, false, false)) {
        Ok(handle) => json_ok(&serde_json::to_string(&handle).unwrap()),
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *const StringHeader {
    let engine = match ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => return json_err("invalid handle"),
    };

    match block(engine.down(&[], false, volumes != 0)) {
        Ok(_) => {
            ComposeEngine::unregister(handle_id as u64);
            json_ok("null")
        },
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *const StringHeader {
    let engine = match ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => return json_err("invalid handle"),
    };

    match block(engine.ps()) {
        Ok(infos) => json_ok(&serde_json::to_string(&infos).unwrap()),
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *const StringHeader {
    let engine = match ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => return json_err("invalid handle"),
    };

    let service = string_from_header(service_ptr);
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    match block(engine.logs(service.as_deref(), tail_opt)) {
        Ok(logs) => json_ok(&serde_json::to_string(&logs).unwrap()),
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *const StringHeader {
    let engine = match ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => return json_err("invalid handle"),
    };

    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => return json_err("service name required"),
    };

    let cmd: Vec<String> = string_from_header(cmd_json_ptr)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    match block(engine.exec(&service, &cmd)) {
        Ok(logs) => json_ok(&serde_json::to_string(&logs).unwrap()),
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => return json_err("invalid spec JSON"),
    };
    let spec = match serde_json::from_str(&json) {
        Ok(s) => s,
        Err(e) => return json_err(&format!("parse error: {}", e)),
    };

    let backend = match block(detect_backend()) {
        Ok(b) => Arc::from(b),
        Err(_) => return json_err("no container backend found"),
    };

    let engine = ComposeEngine::new(spec, "perry".to_string(), backend);
    match engine.config() {
        Ok(yaml) => json_ok(&json!(yaml).to_string()),
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_json_ptr: *const StringHeader) -> *const StringHeader {
    let engine = match ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => return json_err("invalid handle"),
    };
    let services: Vec<String> = string_from_header(services_json_ptr)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    match block(engine.start(&services)) {
        Ok(_) => json_ok("null"),
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut StringHeader {
    let engine = match ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => return json_err("invalid handle") as *mut StringHeader,
    };
    let services: Vec<String> = string_from_header(services_json_ptr)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    match block(engine.stop(&services)) {
        Ok(_) => json_ok("null") as *mut StringHeader,
        Err(e) => json_err(&e.to_string()) as *mut StringHeader,
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut StringHeader {
    let engine = match ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => return json_err("invalid handle") as *mut StringHeader,
    };
    let services: Vec<String> = string_from_header(services_json_ptr)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    match block(engine.restart(&services)) {
        Ok(_) => json_ok("null") as *mut StringHeader,
        Err(e) => json_err(&e.to_string()) as *mut StringHeader,
    }
}
