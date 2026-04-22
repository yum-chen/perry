//! Container module for Perry.
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod context;
pub mod types;
pub mod verification;
pub mod workload;

pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle, ContainerInfo, ContainerLogs,
    ContainerSpec, ImageInfo, ListOrDict,
};

use crate::container::context::{ContainerContext, HandleEntry};
use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::sync::Arc;

/// Helper to create a JS string from a Rust string
unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

// ============ Container Lifecycle ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.run(&spec).await {
            Ok(handle) => {
                let id = ctx.register_handle(HandleEntry::Container(handle));
                Ok(id)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.create(&spec).await {
            Ok(handle) => {
                let id = ctx.register_handle(HandleEntry::Container(handle));
                Ok(id)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        let timeout_opt = if timeout < 0 { None } else { Some(timeout as u32) };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.list(all != 0).await {
            Ok(list) => Ok(ctx.register_handle(HandleEntry::InfoList(list))),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.inspect(&id).await {
            Ok(info) => Ok(info),
            Err(e) => Err::<perry_container_compose::types::ContainerInfo, String>(e.to_string()),
        }
    }, |info| {
        let json = serde_json::to_string(&info).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    // Synchronous probe if not initialized (though async detection is preferred)
    string_to_js("auto")
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        match ctx.get_backend().await {
            Ok(b) => {
                let info = serde_json::json!([{
                    "name": b.backend_name(),
                    "available": true,
                    "mode": "local",
                }]).to_string();
                Ok(info)
            }
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => Ok(ctx.register_handle(HandleEntry::Logs(logs))),
            Err(e) => Err::<u64, String>(e.to_string()),
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
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };
    let cmd_json = types::string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".to_string());
    let env_json = types::string_from_header(env_json_ptr);
    let workdir = types::string_from_header(workdir_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        let env: Option<std::collections::HashMap<String, String>> = env_json.and_then(|s| serde_json::from_str(&s).ok());
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(ctx.register_handle(HandleEntry::Logs(logs))),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match types::string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid reference".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.list_images().await {
            Ok(list) => Ok(ctx.register_handle(HandleEntry::ImageList(list))),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match types::string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid reference".to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

// ============ Compose Orchestration ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match compose::compose_up(spec).await {
            Ok(h) => Ok(h),
            Err(e) => Err::<perry_container_compose::types::ComposeHandle, String>(e),
        }
    }, |h| {
        perry_runtime::JSValue::number(h.stack_id as f64).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_down(handle_id as u64, volumes != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_ps(handle_id as u64).await {
            Ok(list) => {
                let ctx = ContainerContext::global();
                Ok(ctx.register_handle(HandleEntry::InfoList(list)))
            }
            Err(e) => Err::<u64, String>(e),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let service = types::string_from_header(service_ptr);
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_logs(handle_id as u64, service, tail_opt).await {
            Ok(logs) => {
                let ctx = ContainerContext::global();
                Ok(ctx.register_handle(HandleEntry::Logs(logs)))
            }
            Err(e) => Err::<u64, String>(e),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = types::string_from_header(service_ptr).unwrap_or_default();
    let cmd_json = types::string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        match compose::compose_exec(handle_id as u64, service, cmd).await {
            Ok(logs) => {
                let ctx = ContainerContext::global();
                Ok(ctx.register_handle(HandleEntry::Logs(logs)))
            }
            Err(e) => Err::<u64, String>(e),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_config(handle_id as u64).await
    }, |config| {
        let str_ptr = perry_runtime::js_string_from_bytes(config.as_ptr(), config.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = types::string_from_header(services_json_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
        match compose::compose_start(handle_id as u64, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = types::string_from_header(services_json_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
        match compose::compose_stop(handle_id as u64, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = types::string_from_header(services_json_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
        match compose::compose_restart(handle_id as u64, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(handle_id: i64) -> *const StringHeader {
    let ctx = ContainerContext::global();
    match ctx.handles.get(&(handle_id as u64)) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(engine) => {
                let graph = engine.graph().unwrap_or(perry_container_compose::types::ServiceGraph { nodes: vec![], edges: vec![] });
                let json = serde_json::to_string(&graph).unwrap_or_default();
                string_to_js(&json)
            }
            _ => string_to_js("{}"),
        },
        None => string_to_js("{}"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let engine = match ctx.handles.get(&(handle_id as u64)) {
            Some(entry) => match &*entry {
                HandleEntry::Engine(e) => Arc::clone(e),
                _ => return Err("Handle is not an engine".to_string()),
            },
            None => return Err("Handle not found".to_string()),
        };
        match engine.status().await {
            Ok(status) => Ok(status),
            Err(e) => Err::<perry_container_compose::types::StackStatus, String>(e.to_string()),
        }
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Workload Graph ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = types::string_from_header(graph_json_ptr).unwrap_or_else(|| "{}".to_string());
    let _opts_json = types::string_from_header(opts_json_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let graph: workload::WorkloadGraph = serde_json::from_str(&graph_json).map_err(|e| e.to_string())?;
        let id = workload::run_workload_graph(graph).await?;
        Ok(id)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = types::string_from_header(graph_json_ptr).unwrap_or_else(|| "{}".to_string());
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let _graph: workload::WorkloadGraph = serde_json::from_str(&graph_json).map_err(|e| e.to_string())?;
        // Mock status
        Ok(serde_json::json!({"nodes": {}, "healthy": true}).to_string())
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(name_ptr: *const StringHeader, nodes_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = types::string_from_header(name_ptr).unwrap_or_default();
    let nodes_json = types::string_from_header(nodes_json_ptr).unwrap_or_else(|| "{}".to_string());
    let json = serde_json::json!({
        "name": name,
        "nodes": serde_json::from_str::<serde_json::Value>(&nodes_json).unwrap_or_default(),
        "edges": []
    }).to_string();
    string_to_js(&json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = types::string_from_header(name_ptr).unwrap_or_default();
    let spec_json = types::string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".to_string());
    let mut spec = serde_json::from_str::<serde_json::Value>(&spec_json).unwrap_or_default();
    if let Some(obj) = spec.as_object_mut() {
        obj.insert("id".to_string(), serde_json::json!(name.clone()));
        obj.insert("name".to_string(), serde_json::json!(name));
    }
    string_to_js(&spec.to_string())
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: i64, _opts_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_down(handle_id, 0)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: i64) -> *mut Promise {
    js_container_compose_status(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: i64) -> *const StringHeader {
    js_container_compose_graph(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(handle_id: i64, node_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    js_container_compose_logs(handle_id, node_ptr, tail)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(handle_id: i64, node_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_exec(handle_id, node_ptr, cmd_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: i64) -> *mut Promise {
    js_container_compose_ps(handle_id)
}

// ============ Module Initialization ============

#[no_mangle]
pub extern "C" fn js_container_module_init() {
}
