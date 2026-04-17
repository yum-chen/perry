//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::sync::Arc;
use self::backend::get_global_backend;
use self::types::{string_from_header};

/// Helper to create a JS string from a Rust string
unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

// ============ Container Lifecycle ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_composeHandle_down(handle_id: u64, volumes: f64) -> *mut Promise {
    js_container_compose_down(handle_id, volumes)
}

#[no_mangle]
pub unsafe extern "C" fn js_composeHandle_ps(handle_id: u64) -> *mut Promise {
    js_container_compose_ps(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_composeHandle_logs(
    handle_id: u64,
    service_ptr: *const StringHeader,
    tail: f64,
) -> *mut Promise {
    js_container_compose_logs(handle_id, service_ptr, tail)
}

#[no_mangle]
pub unsafe extern "C" fn js_composeHandle_exec(
    handle_id: u64,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_exec(handle_id, service_ptr, cmd_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.create(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().await.map_err(|e| e.to_string())?
            .start(&id).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if timeout < 0.0 { None } else { Some(timeout as u32) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().await.map_err(|e| e.to_string())?
            .stop(&id, t).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().await.map_err(|e| e.to_string())?
            .remove(&id, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().await.map_err(|e| e.to_string())?.list(all != 0.0).await {
            Ok(list) => Ok(types::register_container_info_list(list)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().await.map_err(|e| e.to_string())?.inspect(&id).await {
            Ok(info) => Ok(types::register_container_info(info)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().await.map_err(|e| e.to_string())?.logs(&id, t).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json: *const StringHeader,
    env_json: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_json).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    let env_str = string_from_header(env_json).unwrap_or_default();
    let env: Option<std::collections::HashMap<String, String>> = serde_json::from_str(&env_str).ok();
    let workdir = string_from_header(workdir_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().await.map_err(|e| e.to_string())?
            .exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(image_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().await.map_err(|e| e.to_string())?
            .pull_image(&image).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().await.map_err(|e| e.to_string())?.list_images().await {
            Ok(list) => Ok(types::register_image_info_list(list)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(image_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().await.map_err(|e| e.to_string())?
            .remove_image(&image, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    static BACKEND_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    let name = BACKEND_NAME.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            get_global_backend().await.map(|b| b.backend_name().to_string()).unwrap_or_else(|_| "unknown".to_string())
        })
    });
    string_to_js(name)
}

// ============ Compose Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        let engine = Arc::new(perry_container_compose::ComposeEngine::new(spec, project_name, backend));
        match Arc::clone(&engine).up(&[], true, true, false).await {
            Ok(handle) => Ok(types::register_compose_handle(handle)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: u64, volumes: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                 engine.down(&[], false, volumes != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                match engine.ps().await {
                    Ok(list) => Ok(types::register_container_info_list(list)),
                    Err(e) => Err::<u64, String>(e.to_string()),
                }
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: u64,
    service_ptr: *const StringHeader,
    tail: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr);
    let services = service
        .as_ref()
        .map(|s| vec![s.clone()])
        .unwrap_or_default();
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                match engine.logs(&services, t).await {
                    Ok(logs) => {
                        let combined = logs.values().cloned().collect::<Vec<_>>().join("\n");
                        Ok(types::register_container_logs(types::ContainerLogs {
                            stdout: combined,
                            stderr: String::new(),
                        }))
                    }
                    Err(e) => Err::<u64, String>(e.to_string()),
                }
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: u64,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_ptr).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                match engine.exec(&service, &cmd, None, None).await {
                    Ok(res) => Ok(types::register_container_logs(types::ContainerLogs {
                        stdout: res.stdout,
                        stderr: res.stderr,
                    })),
                    Err(e) => Err::<u64, String>(e.to_string()),
                }
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                match serde_json::to_string(&engine.spec) {
                    Ok(json) => Ok(types::register_string(json)),
                    Err(e) => Err::<u64, String>(e.to_string()),
                }
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                engine.start(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                engine.stop(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                engine.restart(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
            } else {
                Err::<u64, String>("Compose engine not found for handle".to_string())
            }
        } else {
            Err::<u64, String>("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend::detect_backend().await {
            Ok(_) => {
                 // Return detection info
                 let backend = backend::get_global_backend().await.map_err(|e| e.to_string())?;
                 let name = backend.backend_name();
                 let json = serde_json::json!([{
                     "name": name,
                     "available": true,
                 }]);
                 Ok(types::register_string(json.to_string()))
            }
            Err(probed) => {
                let json = serde_json::to_string(&probed).unwrap_or_else(|_| "[]".to_string());
                Ok(types::register_string(json))
            }
        }
    });
    promise
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // Pre-initialize backend
    let _ = tokio::runtime::Handle::current().spawn(async {
        let _ = get_global_backend().await;
    });
}
