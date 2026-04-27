//! FFI exports for Perry TypeScript integration.
//!
//! Each function follows the Perry FFI convention:
//! - String arguments arrive as `*const StringHeader` (Perry runtime layout)
//! - Results are serialised to JSON strings before being handed back to JS

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

fn parse_compose_file(file_ptr: *const StringHeader) -> Option<PathBuf> {
    unsafe { string_from_header(file_ptr) }.map(PathBuf::from)
}

fn make_engine(files: Vec<PathBuf>) -> Result<Arc<ComposeEngine>, String> {
    let proj = crate::project::ComposeProject::load_from_files(&files, None, &[])
        .map_err(|e| e.to_string())?;
    let backend = block(crate::backend::detect_backend())
        .map_err(|e| {
            let err = crate::error::ComposeError::NoBackendFound { probed: e };
            crate::error::compose_error_to_js(&err)
        })?;
    Ok(Arc::new(ComposeEngine::new(proj.spec, proj.project_name, backend)))
}

// ──────────────────────────────────────────────────────────────
// Exported FFI functions
// ──────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    match make_engine(files) {
        Err(e) => json_err(&e),
        Ok(engine) => match block(engine.up(&[], true, false, false)) {
            Ok(_) => json_ok("null"),
            Err(e) => json_err(&compose_error_to_js_internal(&e)),
        },
    }
}

fn compose_error_to_js_internal(e: &crate::error::ComposeError) -> String {
    crate::error::compose_error_to_js(e)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    match make_engine(files) {
        Err(e) => json_err(&e),
        Ok(engine) => match block(engine.down(&[], false, false)) {
            Ok(_) => json_ok("null"),
            Err(e) => json_err(&compose_error_to_js_internal(&e)),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    match make_engine(files) {
        Err(e) => json_err(&e),
        Ok(engine) => match block(engine.ps()) {
            Err(e) => json_err(&compose_error_to_js_internal(&e)),
            Ok(infos) => {
                let items: Vec<String> = infos
                    .iter()
                    .map(|i| {
                        format!(
                            "{{\"service\":\"{}\",\"container\":\"{}\",\"status\":\"{}\"}}",
                            i.name, i.id, i.status
                        )
                    })
                    .collect();
                let array = format!("[{}]", items.join(","));
                json_ok(&array)
            }
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(
    file_ptr: *const StringHeader,
    services_ptr: *const StringHeader,
    _follow: bool,
) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    let services: Vec<String> = string_from_header(services_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();

    match make_engine(files) {
        Err(e) => json_err(&e),
        Ok(engine) => match block(engine.logs(&services, None)) {
            Err(e) => json_err(&compose_error_to_js_internal(&e)),
            Ok(logs_map) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                for (svc, logs) in logs_map {
                    stdout.push_str(&format!("[{}] {}\n", svc, logs.stdout));
                    stderr.push_str(&format!("[{}] {}\n", svc, logs.stderr));
                }
                let out_esc = stdout.replace('"', "\\\"").replace('\n', "\\n");
                let err_esc = stderr.replace('"', "\\\"").replace('\n', "\\n");
                let payload = format!("{{\"stdout\":\"{}\",\"stderr\":\"{}\"}}", out_esc, err_esc);
                json_ok(&payload)
            }
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    file_ptr: *const StringHeader,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => return json_err("service name is required"),
    };
    let cmd: Vec<String> = string_from_header(cmd_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();

    match make_engine(files) {
        Err(e) => json_err(&e),
        Ok(engine) => match block(engine.exec(&service, &cmd)) {
            Err(e) => json_err(&compose_error_to_js_internal(&e)),
            Ok(result) => {
                let stdout = result.stdout.replace('"', "\\\"").replace('\n', "\\n");
                let stderr = result.stderr.replace('"', "\\\"").replace('\n', "\\n");
                let payload = format!(
                    "{{\"stdout\":\"{}\",\"stderr\":\"{}\"}}",
                    stdout, stderr
                );
                json_ok(&payload)
            }
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    match crate::project::ComposeProject::load_from_files(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(proj) => {
            let yaml = proj.spec.to_yaml().unwrap_or_default();
            let escaped = yaml.replace('"', "\\\"").replace('\n', "\\n");
            json_ok(&format!("\"{}\"", escaped))
        }
    }
}
