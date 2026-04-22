//! Perry container module FFI bridge.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod workload;
pub mod types;
pub mod verification;

use perry_container_compose::backend::{detect_backend, ContainerBackend};
use perry_container_compose::error::compose_error_to_js;
use perry_container_compose::ComposeEngine;
use perry_runtime::{js_promise_new, Promise, StringHeader, JSValue};
use std::sync::{Arc, OnceLock};
use crate::container::types::*;
use crate::common::spawn_for_promise_deferred;
use dashmap::DashMap;

pub(crate) mod mod_private {
    use super::*;
    use tokio::sync::Mutex;

    pub static BACKEND: OnceLock<Arc<dyn ContainerBackend + Send + Sync>> = OnceLock::new();
    static INIT_MUTEX: Mutex<()> = Mutex::const_new(());

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }

        let _guard = INIT_MUTEX.lock().await;
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }

        let backend_res = detect_backend().await;

        match backend_res {
            Ok(b) => {
                let _ = BACKEND.set(Arc::clone(&b));
                Ok(b)
            }
            Err(probed) => Err(format!("No backend found: {:?}", probed)),
        }
    }
}

use mod_private::get_global_backend_instance;

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    if spec_json_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null spec JSON pointer".to_string()) });
        return promise;
    }
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid spec JSON".to_string()) });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(format!("Invalid ContainerSpec: {}", e)) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let internal_spec = perry_container_compose::types::ContainerSpec {
            image: spec.image,
            name: spec.name,
            ports: spec.ports,
            volumes: spec.volumes,
            env: spec.env,
            labels: spec.labels,
            cmd: spec.cmd,
            entrypoint: spec.entrypoint,
            network: spec.network,
            rm: spec.rm,
            read_only: spec.read_only,
            seccomp: spec.seccomp,
        };
        let handle = backend.run(&internal_spec).await.map_err(|e| compose_error_to_js(&e))?;
        let id = register_container_handle(ContainerHandle { id: handle.id, name: handle.name });
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    if spec_json_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null spec JSON pointer".to_string()) });
        return promise;
    }
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid spec JSON".to_string()) });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(format!("Invalid ContainerSpec: {}", e)) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let internal_spec = perry_container_compose::types::ContainerSpec {
            image: spec.image,
            name: spec.name,
            ports: spec.ports,
            volumes: spec.volumes,
            env: spec.env,
            labels: spec.labels,
            cmd: spec.cmd,
            entrypoint: spec.entrypoint,
            network: spec.network,
            rm: spec.rm,
            read_only: spec.read_only,
            seccomp: spec.seccomp,
        };
        let handle = backend.create(&internal_spec).await.map_err(|e| compose_error_to_js(&e))?;
        let id = register_container_handle(ContainerHandle { id: handle.id, name: handle.name });
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    if id_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null ID pointer".to_string()) });
        return promise;
    }
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID string".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.start(&id).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();
    if id_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null ID pointer".to_string()) });
        return promise;
    }
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID string".to_string()) });
            return promise;
        }
    };

    let t = if timeout >= 0.0 { Some(timeout as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.stop(&id, t).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    if id_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null ID pointer".to_string()) });
        return promise;
    }
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID string".to_string()) });
            return promise;
        }
    };

    let f = force != 0.0;

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove(&id, f).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    let a = all != 0.0;
    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.list(a).await.map_err(|e| compose_error_to_js(&e))
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    if id_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null ID pointer".to_string()) });
        return promise;
    }
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID string".to_string()) });
            return promise;
        }
    };

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.inspect(&id).await.map_err(|e| compose_error_to_js(&e))
    }, |info| {
        let json = serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    if id_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null ID pointer".to_string()) });
        return promise;
    }
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID string".to_string()) });
            return promise;
        }
    };

    let t = if tail >= 0.0 { Some(tail as u32) } else { None };

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.logs(&id, t).await.map_err(|e| compose_error_to_js(&e))
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader
) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };
    let cmd: Vec<String> = match string_from_header(cmd_json_ptr).and_then(|s| serde_json::from_str(&s).ok()) {
        Some(v) => v,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid cmd JSON".to_string()) });
            return promise;
        }
    };
    let env: Option<std::collections::HashMap<String, String>> = string_from_header(env_json_ptr).and_then(|s| serde_json::from_str(&s).ok());
    let workdir = string_from_header(workdir_ptr);

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| compose_error_to_js(&e))
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid image ref".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.list_images().await.map_err(|e| compose_error_to_js(&e))
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid image ref".to_string()) });
            return promise;
        }
    };
    let f = force != 0.0;

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove_image(&reference, f).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = if let Some(backend) = mod_private::BACKEND.get() {
        backend.backend_name()
    } else {
        "unknown"
    };
    perry_runtime::js_string_from_bytes(name.as_ptr(), name.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise_deferred(promise as *mut u8, async move {
        match detect_backend().await {
            Ok(backend) => {
                let name = backend.backend_name().to_string();
                let _ = mod_private::BACKEND.set(Arc::clone(&backend));
                Ok(vec![perry_container_compose::error::BackendProbeResult {
                    name,
                    available: true,
                    reason: String::new(),
                }])
            }
            Err(probed) => Ok(probed),
        }
    }, |probed| {
        let json = serde_json::to_string(&probed).unwrap_or_else(|_| "[]".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    if spec_json_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Null spec pointer".to_string()) });
        return promise;
    }
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid spec JSON".to_string()) });
            return promise;
        }
    };

    let spec: perry_container_compose::types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(format!("Invalid ComposeSpec: {}", e)) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let handle = compose::compose_up(spec).await.map_err(|e| e.to_string())?;
        Ok(handle.stack_id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: f64, volumes: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let v = volumes != 0.0;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        compose::compose_down(id, v).await.map(|_| 0).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: f64, volumes: f64) -> *mut Promise {
    js_container_compose_down(handle_id, volumes)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_ps(id).await
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: f64) -> *mut Promise {
    js_container_compose_ps(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: f64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let service = string_from_header(service_ptr);
    let t = if tail >= 0.0 { Some(tail as u32) } else { None };

    spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_logs(id, service, t).await
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(handle_id: f64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    js_container_compose_logs(handle_id, service_ptr, tail)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: f64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    opts_json_ptr: *const StringHeader
) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid service name".to_string()) });
            return promise;
        }
    };
    let cmd: Vec<String> = match string_from_header(cmd_json_ptr).and_then(|s| serde_json::from_str(&s).ok()) {
        Some(v) => v,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid cmd JSON".to_string()) });
            return promise;
        }
    };

    let opts: serde_json::Value = string_from_header(opts_json_ptr)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);

    let env: Option<std::collections::HashMap<String, String>> = opts.get("env")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let workdir = opts.get("workdir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_exec(id, service, cmd, env, workdir).await
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle_id: f64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    opts_json_ptr: *const StringHeader
) -> *mut Promise {
    js_container_compose_exec(handle_id, service_ptr, cmd_json_ptr, opts_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_config(id).await
    }, |config| {
        let str_ptr = perry_runtime::js_string_from_bytes(config.as_ptr(), config.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(handle_id: f64) -> *mut Promise {
    js_container_compose_config(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services: Vec<String> = string_from_header(services_json_ptr).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        compose::compose_start(id, services).await.map(|_| 0).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_start(handle_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services: Vec<String> = string_from_header(services_json_ptr).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        compose::compose_stop(id, services).await.map(|_| 0).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_stop(handle_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services: Vec<String> = string_from_header(services_json_ptr).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        compose::compose_restart(id, services).await.map(|_| 0).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_restart(handle_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_build(spec_json_ptr: *const StringHeader, image_name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid spec JSON".to_string()) });
            return promise;
        }
    };
    let image_name = match string_from_header(image_name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid image name".to_string()) });
            return promise;
        }
    };

    let spec: perry_container_compose::types::ComposeServiceBuild = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(format!("Invalid build spec: {}", e)) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.build(&spec, &image_name).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(_name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    // Shorthand for serializing a WorkloadGraph
    let json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".to_string());
    perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_json_ptr).unwrap_or_default();
    let opts_json = string_from_header(opts_json_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let engine = perry_container_compose::compose::WorkloadGraphEngine::new(backend);
        engine.run(&graph_json, &opts_json).await.map_err(|e| e.to_string())
    });
    promise
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    #[test]
    fn test_smoke_module_init() {
        // Just verify it doesn't panic
        unsafe {
            let _ = js_container_getBackend();
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: f64, _opts_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_down(handle_id, 0.0) // Shorthand
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: f64) -> *mut Promise {
    js_container_compose_ps(handle_id) // Shorthand
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(_name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".to_string());
    perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_json_ptr).unwrap_or_default();
    spawn_for_promise_deferred(promise as *mut u8, async move {
        let spec: perry_container_compose::types::ComposeSpec = serde_json::from_str(&graph_json).map_err(|e| e.to_string())?;
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let engine = ComposeEngine::new(spec, "inspect".to_string(), backend);
        engine.status().await.map_err(|e| e.to_string())
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: f64) -> *const StringHeader {
    js_container_compose_graph(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(handle_id: f64, node_ptr: *const StringHeader, _opts_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_logs(handle_id, node_ptr, 0.0)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(handle_id: f64, node_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_exec(handle_id, node_ptr, cmd_json_ptr, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: f64) -> *mut Promise {
    js_container_compose_ps(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_module_init() {
    // Initialise the container module
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(handle_id: f64) -> *const StringHeader {
    let id = handle_id as u64;
    let json = if let Some(engine) = COMPOSE_HANDLES.get_or_init(DashMap::new).get(&id) {
        if let Ok(graph) = engine.0.graph() {
            serde_json::to_string(&graph).unwrap_or_else(|_| "{}".to_string())
        } else {
            "{}".to_string()
        }
    } else {
        "{}".to_string()
    };
    perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get_or_init(DashMap::new)
            .get(&id)
            .map(|e| Arc::clone(&e.0))
            .ok_or_else(|| format!("Compose stack {} not found", id))?;
        engine.status().await.map_err(|e| e.to_string())
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}
