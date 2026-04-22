//! Container module for Perry

pub mod backend;
pub mod compose;
pub mod types;
pub mod verification;
pub mod capability;

// Shared internal module to avoid circular dependency between mod.rs and capability.rs
pub(crate) mod mod_inner {
    use std::sync::{Arc, OnceLock};
    use super::backend::{detect_backend, ContainerBackend};

    static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }
        match detect_backend().await {
            Ok(b) => {
                let arc_b: Arc<dyn ContainerBackend> = Arc::from(b);
                let _ = BACKEND.set(Arc::clone(&arc_b));
                Ok(arc_b)
            }
            Err(probed) => Err(format!("No container backend found. Probed: {:?}", probed)),
        }
    }

    pub fn get_global_backend_sync() -> Option<Arc<dyn ContainerBackend>> {
        BACKEND.get().map(Arc::clone)
    }
}

pub use types::{
    ComposeHealthcheck, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume,
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeError,
};

use perry_runtime::{js_promise_new, Promise, StringHeader, JSValue};
pub use backend::ContainerBackend;
use std::sync::{Arc, OnceLock};
use mod_inner::{get_global_backend_instance, get_global_backend_sync};
use dashmap::DashMap;

static COMPOSE_ENGINES: OnceLock<DashMap<u64, perry_container_compose::compose::ComposeEngine>> = OnceLock::new();

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

fn compose_error_to_js(e: ComposeError) -> String {
    let code = match &e {
        ComposeError::NotFound(_) => 404,
        ComposeError::BackendError { code, .. } => *code,
        ComposeError::NoBackendFound { .. } => 503,
        ComposeError::BackendNotAvailable { .. } => 503,
        ComposeError::DependencyCycle { .. } => 422,
        ComposeError::ServiceStartupFailed { .. } => 500,
        ComposeError::ParseError(_) => 400,
        ComposeError::JsonError(_) => 400,
        ComposeError::IoError(_) => 500,
        ComposeError::ValidationError { .. } => 400,
        ComposeError::VerificationFailed { .. } => 403,
        ComposeError::FileNotFound { .. } => 404,
    };
    serde_json::json!({
        "message": e.to_string(),
        "code": code
    }).to_string()
}

fn backend_err_to_js(msg: String) -> String {
    serde_json::json!({
        "message": msg,
        "code": 503
    }).to_string()
}

// Helpers to return JSON strings to Perry FFI
unsafe fn json_to_js<T: serde::Serialize>(val: T) -> u64 {
    let json = serde_json::to_string(&val).unwrap_or_default();
    string_to_js(&json) as u64
}

// ============ Container API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.create(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.stop(&id, if timeout >= 0 { Some(timeout as u32) } else { None }).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.list(all != 0).await {
            Ok(list) => Ok(json_to_js(list)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.inspect(&id).await {
            Ok(info) => Ok(json_to_js(info)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.logs(&id, if tail >= 0 { Some(tail as u32) } else { None }).await {
            Ok(logs) => Ok(json_to_js(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let env_json = string_from_header(env_json_ptr).unwrap_or_else(|| "{}".into());
    let workdir = string_from_header(workdir_ptr);

    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
    let env: Option<std::collections::HashMap<String, String>> = serde_json::from_str(&env_json).ok();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(json_to_js(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid reference".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.list_images().await {
            Ok(list) => Ok(json_to_js(list)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid reference".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = if let Some(b) = get_global_backend_sync() {
        b.backend_name().to_string()
    } else {
        match std::env::consts::OS {
            "macos" | "ios" => "apple/container".into(),
            _ => "podman".into(),
        }
    };
    string_to_js(&name)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend::detect_backend().await {
            Ok(b) => {
                let info = serde_json::json!([{
                    "name": b.backend_name(),
                    "available": true
                }]);
                Ok(json_to_js(info))
            }
            Err(probed) => {
                Ok(json_to_js(probed))
            }
        }
    });
    promise
}

// ============ Compose API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let project_name = spec.name.clone().unwrap_or_else(|| "default".into());
        let engine = perry_container_compose::compose::ComposeEngine::new(spec, project_name, backend);
        match engine.up(&[], false, false, false).await {
            Ok(handle) => {
                let id = handle.stack_id;
                COMPOSE_ENGINES.get_or_init(DashMap::new).insert(id, engine);
                Ok(types::register_compose_handle_ffi(handle))
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(stack_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.down(&[], false, volumes != 0).await {
            Ok(()) => {
                COMPOSE_ENGINES.get().map(|m| m.remove(&id));
                Ok(0u64)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(stack_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.ps().await {
            Ok(list) => Ok(json_to_js(list)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(stack_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let service = if service_ptr.is_null() { None } else { string_from_header(service_ptr) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        let services = service.map(|s| vec![s]).unwrap_or_default();
        match engine.logs(&services, if tail >= 0 { Some(tail as u32) } else { None }).await {
            Ok(logs_map) => Ok(json_to_js(logs_map)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(stack_id: i64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid service name".into())) });
            return promise;
        }
    };
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.exec(&service, &cmd, None, None).await {
            Ok(logs) => Ok(json_to_js(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".into());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        Ok(json_to_js(spec_json))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.start(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.stop(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.restart(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(stack_id: i64, volumes: i32) -> *mut Promise {
    js_compose_down(stack_id, volumes)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(stack_id: i64) -> *mut Promise {
    js_compose_ps(stack_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(stack_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    js_compose_logs(stack_id, service_ptr, tail)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(stack_id: i64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_exec(stack_id, service_ptr, cmd_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_config(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_start(stack_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_stop(stack_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_restart(stack_id, services_json_ptr)
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    tokio::spawn(async {
        let _ = get_global_backend_instance().await;
    });
}
