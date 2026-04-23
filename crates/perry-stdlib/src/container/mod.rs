//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

// Re-export commonly used types
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

use perry_runtime::{js_promise_new, Promise, StringHeader, JSValue};
pub use backend::{detect_backend, ContainerBackend};
use std::sync::OnceLock;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: Mutex<()> = Mutex::const_new(());

/// Get or initialize the global backend instance
async fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    let _guard = BACKEND_INIT_MUTEX.lock().await;

    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    let b = detect_backend().await
        .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
        .map_err(|probed| ContainerError::NoBackendFound { probed })?;

    let _ = BACKEND.set(Arc::clone(&b));
    Ok(b)
}

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

/// Helper to create a JS string from a Rust string
unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

// ============ Container Lifecycle ============

/// Run a container from the given spec
/// FFI: js_container_run(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.run(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Create a container from the given spec without starting it
/// FFI: js_container_create(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.create(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Start a previously created container
/// FFI: js_container_start(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, timeout: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout_opt = if timeout < 0 { None } else { Some(timeout as u32) };
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(all: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<String, String>(e.to_string()),
        };
        match backend.list(all != 0).await {
            Ok(containers) => {
                serde_json::to_string(&containers).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Inspect a container
/// FFI: js_container_inspect(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<String, String>(e.to_string()),
        };
        match backend.inspect(&id).await {
            Ok(info) => {
                serde_json::to_string(&info).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Get the current backend name
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    // Note: this is synchronous and might return "unknown" if not initialized
    if let Some(b) = BACKEND.get() {
        return string_to_js(b.backend_name());
    }
    string_to_js("unknown")
}

/// Detect backend and return probed info
/// FFI: js_container_detectBackend() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let (_, results) = perry_container_compose::backend::probe_all_backends().await;
        Ok(serde_json::to_string(&results).unwrap_or_default())
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Container Logs and Exec ============

/// Get logs from a container
/// FFI: js_container_logs(id: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<String, String>(e.to_string()),
        };
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => {
                serde_json::to_string(&logs).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Build an image from a build spec
/// FFI: js_container_build(spec_json: *const StringHeader, image_name: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_build(spec_ptr: *const StringHeader, image_name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = string_from_header(spec_ptr);
    let image_name = string_from_header(image_name_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: perry_container_compose::types::ComposeServiceBuild = spec_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or_else(|| "Invalid build spec".to_string())?;
        let name = image_name.ok_or_else(|| "Invalid image name".to_string())?;

        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        backend.build(&spec, &name).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// Execute a command in a container
/// FFI: js_container_exec(id: *const StringHeader, cmd_json: *const StringHeader, env_json: *const StringHeader, workdir: *const StringHeader) -> *mut Promise
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
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let cmd_json = string_from_header(cmd_json_ptr);
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<String, String>(e.to_string()),
        };
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => {
                serde_json::to_string(&logs).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

// ============ Image Management ============

/// Pull a container image
/// FFI: js_container_pullImage(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List images
/// FFI: js_container_listImages() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<String, String>(e.to_string()),
        };
        match backend.list_images().await {
            Ok(images) => {
                serde_json::to_string(&images).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Remove an image
/// FFI: js_container_removeImage(reference: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_compose_up(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper = compose::ComposeWrapper::new(spec, backend);
        match wrapper.up().await {
            Ok(handle) => {
                let handle_id = types::register_compose_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Alias for js_compose_up (for perry/container import)
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
    js_compose_up(spec_ptr)
}

/// Stop and remove compose stack.
/// FFI: js_compose_down(handle_id: i64, volumes: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::take_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = match compose::get_engine_wrapper(handle.stack_id) {
            Some(w) => w,
            None => return Err::<u64, String>("Compose engine not found".to_string()),
        };
        match wrapper.down(&handle, volumes != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get container info for compose stack
/// FFI: js_compose_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let wrapper = match compose::get_engine_wrapper(handle.stack_id) {
            Some(w) => w,
            None => return Err::<String, String>("Compose engine not found".to_string()),
        };
        match wrapper.ps(&handle).await {
            Ok(containers) => {
                serde_json::to_string(&containers).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_compose_logs(handle_id: i64, service: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service = unsafe { string_from_header(service_ptr) };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };
        let wrapper = match compose::get_engine_wrapper(handle.stack_id) {
            Some(w) => w,
            None => return Err::<String, String>("Compose engine not found".to_string()),
        };
        match wrapper.logs(&handle, service.as_deref(), tail_opt).await {
            Ok(logs) => {
                serde_json::to_string(&logs).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Execute command in compose service
/// FFI: js_compose_exec(handle_id: i64, service: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service_opt = unsafe { string_from_header(service_ptr) };
    let cmd_json = unsafe { string_from_header(cmd_json_ptr) };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err::<String, String>("Invalid service name".to_string()),
        };

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let wrapper = match compose::get_engine_wrapper(handle.stack_id) {
            Some(w) => w,
            None => return Err::<String, String>("Compose engine not found".to_string()),
        };
        match wrapper.exec(&handle, &service, &cmd).await {
            Ok(logs) => {
                serde_json::to_string(&logs).map_err(|e| e.to_string())
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Start compose services
/// FFI: js_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_start(
    handle_id: i64,
    services_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };
    let services_json = string_from_header(services_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let wrapper = match compose::get_engine_wrapper(handle.stack_id) {
            Some(w) => w,
            None => return Err::<u64, String>("Compose engine not found".to_string()),
        };
        wrapper.start(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// Stop compose services
/// FFI: js_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(
    handle_id: i64,
    services_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };
    let services_json = string_from_header(services_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let wrapper = match compose::get_engine_wrapper(handle.stack_id) {
            Some(w) => w,
            None => return Err::<u64, String>("Compose engine not found".to_string()),
        };
        wrapper.stop(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// Restart compose services
/// FFI: js_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(
    handle_id: i64,
    services_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };
    let services_json = string_from_header(services_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let wrapper = match compose::get_engine_wrapper(handle.stack_id) {
            Some(w) => w,
            None => return Err::<u64, String>("Compose engine not found".to_string()),
        };
        wrapper.restart(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// Get compose configuration
/// FFI: js_compose_config(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<String, String>(e.to_string()),
        };
        let wrapper = compose::ComposeWrapper::new(spec, backend);
        wrapper.config().map_err(|e| e.to_string())
    }, |yaml| {
        let str_ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
}
