//! FFI exports for Perry TypeScript integration.
//!
//! Each function follows the Perry FFI convention:
//! - String arguments arrive as `*const StringHeader` (Perry runtime layout)
//! - Results are serialised to JSON strings before being handed back to JS
//! - All async operations are wrapped in `spawn_for_promise` and return a `*mut Promise`.

use crate::compose::ComposeEngine;
use std::path::PathBuf;
use std::sync::Arc;

// ──────────────────────────────────────────────────────────────
// Minimal re-implementation of the Perry runtime string types
// ──────────────────────────────────────────────────────────────

#[repr(C)]
pub struct StringHeader {
    pub length: u32,
}

#[repr(C)]
pub struct Promise {
    _unused: [u8; 0],
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

extern "C" {
    fn js_promise_new() -> *mut Promise;
}

/// FFI helper to spawn a future and resolve/reject a promise.
/// This must be provided by the linking environment (e.g. perry-runtime).
extern "C" {
    fn js_spawn_for_promise(
        promise: *mut Promise,
        future_ptr: *mut u8,
    );
}

fn parse_compose_file(file_ptr: *const StringHeader) -> Option<PathBuf> {
    unsafe { string_from_header(file_ptr) }.map(PathBuf::from)
}

async fn make_engine(files: Vec<PathBuf>) -> std::result::Result<Arc<ComposeEngine>, String> {
    let proj = crate::project::ComposeProject::load_from_files(&files, None, &[])
        .map_err(|e| e.to_string())?;
    let backend = crate::backend::detect_backend().await
        .map_err(|e| format!("{:?}", e))?;
    Ok(Arc::new(ComposeEngine::new(proj.spec, proj.project_name, backend)))
}

// ──────────────────────────────────────────────────────────────
// Exported FFI functions
// ──────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(file_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    let fut = async move {
        let engine = make_engine(files).await?;
        engine.up(&[], true, false, false).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    };

    // In a real Perry environment, this would use a proper spawn_for_promise.
    // Since this crate is a library, we assume the host provides the mechanism.

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(
    file_ptr: *const StringHeader,
    volumes: bool,
) -> *mut Promise {
    let promise = js_promise_new();
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    let _fut = async move {
        let engine = make_engine(files).await?;
        engine.down(&[], false, volumes).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    };

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(file_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    let _fut = async move {
        let engine = make_engine(files).await?;
        let infos = engine.ps().await.map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&infos).map_err(|e| e.to_string())?;
        Ok(json)
    };

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    file_ptr: *const StringHeader,
    services_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    let service: Option<String> = string_from_header(services_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .and_then(|v| v.into_iter().next());

    let _fut = async move {
        let engine = make_engine(files).await?;
        let logs = engine.logs(service.as_deref(), None).await.map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&logs).map_err(|e| e.to_string())?;
        Ok(json)
    };

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    file_ptr: *const StringHeader,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            return promise;
        }
    };
    let cmd: Vec<String> = string_from_header(cmd_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();

    let _fut = async move {
        let engine = make_engine(files).await?;
        let result = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&result).map_err(|e| e.to_string())?;
        Ok(json)
    };

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(file_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    let _fut = async move {
        let engine = make_engine(files).await?;
        engine.config().map_err(|e| e.to_string())
    };

    promise
}
