//! Container module for Perry

pub mod backend;
pub mod capability;
pub mod compose;
pub mod context;
pub mod types;
pub mod verification;
pub mod workload;

pub(crate) mod mod_priv {
    use super::context::ContainerContext;
    use perry_container_compose::backend::ContainerBackend;
    use std::sync::Arc;

    pub async fn get_global_backend_instance() -> Arc<dyn ContainerBackend> {
        ContainerContext::global()
            .get_backend()
            .await
            .expect("No container backend found")
    }
}

use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::collections::HashMap;
use std::sync::Arc;
use self::mod_priv::get_global_backend_instance;
use perry_container_compose::error::ComposeError;

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

pub unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

pub unsafe fn parse_workload_graph_json(ptr: *const StringHeader) -> Result<perry_container_compose::types::WorkloadGraph, String> {
    let s = string_from_header(ptr).ok_or("Invalid graph pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub fn compose_error_to_js(e: &ComposeError) -> String {
    let code = match &e {
        ComposeError::NotFound(_) => 404,
        ComposeError::BackendError { code, .. } => *code,
        ComposeError::DependencyCycle { .. } => 422,
        ComposeError::ValidationError { .. } => 400,
        ComposeError::VerificationFailed { .. } => 403,
        _ => 500,
    };
    serde_json::json!({
        "message": e.to_string(),
        "code": code
    }).to_string()
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
        let backend = get_global_backend_instance().await;
        match backend.run(&spec.into()).await {
            Ok(handle) => Ok(types::register_container_handle(handle.into())),
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
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
        let backend = get_global_backend_instance().await;
        match backend.create(&spec.into()).await {
            Ok(handle) => Ok(types::register_container_handle(handle.into())),
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend_instance()
            .await
            .start(&id)
            .await
            .map(|_| 0u64)
            .map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if timeout < 0.0 { None } else { Some(timeout as u32) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend_instance()
            .await
            .stop(&id, t)
            .await
            .map(|_| 0u64)
            .map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend_instance()
            .await
            .remove(&id, force != 0.0)
            .await
            .map(|_| 0u64)
            .map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend_instance().await.list(all != 0.0).await {
            Ok(list) => Ok(types::register_container_info_list(
                list.into_iter().map(|i| i.into()).collect(),
            )),
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend_instance().await.inspect(&id).await {
            Ok(info) => Ok(types::register_container_info(info.into())),
            Err(e) => Err(compose_error_to_js(&e)),
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
        match get_global_backend_instance().await.logs(&id, t).await {
            Ok(logs) => Ok(types::register_container_logs(logs.into())),
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_json).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend_instance()
            .await
            .exec(&id, &cmd, None, None)
            .await
        {
            Ok(logs) => Ok(types::register_container_logs(logs.into())),
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_build(
    spec_json: *const StringHeader,
    image_name_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let spec_str = string_from_header(spec_json).unwrap_or_default();
    let spec: perry_container_compose::types::ComposeServiceBuild = serde_json::from_str(&spec_str).unwrap_or_default();
    let image_name = string_from_header(image_name_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await;
        backend
            .build(&spec, &image_name)
            .await
            .map(|_| 0u64)
            .map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(image_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend_instance()
            .await
            .pull_image(&image)
            .await
            .map(|_| 0u64)
            .map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend_instance().await.list_images().await {
            Ok(list) => Ok(types::register_image_info_list(
                list.into_iter().map(|i| i.into()).collect(),
            )),
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(
    image_ptr: *const StringHeader,
    force: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend_instance()
            .await
            .remove_image(&image, force != 0.0)
            .await
            .map(|_| 0u64)
            .map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Some(backend) = context::ContainerContext::global().try_get_backend() {
        return string_to_js(backend.backend_name());
    }
    string_to_js("unknown")
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let results = perry_container_compose::backend::probe_all_candidates().await;
        let infos: Vec<serde_json::Value> = results.into_iter().map(|r| {
            serde_json::json!({
                "name": r.name,
                "available": r.available,
                "reason": if r.available { None } else { Some(r.reason) },
                "version": None::<String>,
                "mode": "local",
                "isolationLevel": "container",
            })
        }).collect();
        Ok(types::register_string(serde_json::to_string(&infos).unwrap()))
    });
    promise
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
        let backend = get_global_backend_instance().await;
        let project_name = spec
            .name
            .clone()
            .unwrap_or_else(|| "perry-stack".to_string());
        let engine = Arc::new(perry_container_compose::ComposeEngine::new(
            spec,
            project_name,
            backend,
        ));
        match engine.up(&[], true, true, false).await {
            Ok(handle) => Ok(types::register_compose_engine(engine, handle.stack_id)),
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: u64, volumes: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.down(volumes != 0.0).await.map(|_| 0u64).map_err(|e| compose_error_to_js(&e))
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.ps().await {
                Ok(list) => Ok(types::register_container_info_list(
                    list.into_iter().map(|i| i.into()).collect(),
                )),
                Err(e) => Err(compose_error_to_js(&e)),
            }
        } else {
            Err("Invalid compose handle".to_string())
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
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.logs(service.as_deref(), t).await {
                Ok(logs) => {
                    Ok(types::register_container_logs(logs.into()))
                }
                Err(e) => Err(compose_error_to_js(&e)),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: u64,
    service_ptr: *const StringHeader,
    cmd_json: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_json).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.exec(&service, &cmd).await {
                Ok(res) => Ok(types::register_container_logs(res.into())),
                Err(e) => Err(compose_error_to_js(&e)),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.config() {
                Ok(yaml) => Ok(types::register_string(yaml)),
                Err(e) => Err(compose_error_to_js(&e)),
            }
        } else {
            Err("Invalid compose handle".to_string())
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
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.start(&services).await.map(|_| 0u64).map_err(|e| compose_error_to_js(&e))
        } else {
            Err("Invalid compose handle".to_string())
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
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.stop(&services).await.map(|_| 0u64).map_err(|e| compose_error_to_js(&e))
        } else {
            Err("Invalid compose handle".to_string())
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
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.restart(&services).await.map(|_| 0u64).map_err(|e| compose_error_to_js(&e))
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(handle_id: u64) -> *const StringHeader {
    if let Some(engine) = types::get_compose_engine(handle_id) {
        let nodes: Vec<String> = engine.spec.services.keys().cloned().collect();
        let edges: Vec<serde_json::Value> = vec![]; // Simplified
        let graph = serde_json::json!({
            "nodes": nodes,
            "edges": edges,
        });
        return string_to_js(&graph.to_string());
    }
    string_to_js("{}")
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            let mut services = vec![];
            let mut healthy = true;
            for (id, svc) in &engine.spec.services {
                let container_name = perry_container_compose::service::Service::generate_name(
                    svc.image.as_deref().unwrap_or("unknown"), id);
                let (state, error) = if let Ok(info) = engine.backend.inspect(&container_name).await {
                    if info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up") {
                        ("running".to_string(), None::<String>)
                    } else {
                        healthy = false;
                        ("stopped".to_string(), None::<String>)
                    }
                } else {
                    healthy = false;
                    ("unknown".to_string(), None::<String>)
                };
                services.push(serde_json::json!({
                    "service": id,
                    "state": state,
                    "error": error,
                }));
            }
            let status = serde_json::json!({
                "services": services,
                "healthy": healthy,
            });
            Ok(types::register_string(status.to_string()))
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

// ============ Workload Graph Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(
    name_ptr: *const StringHeader,
    spec_json: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_str = string_from_header(spec_json).unwrap_or_default();
    let spec: serde_json::Value = serde_json::from_str(&spec_str).unwrap_or_default();

    let graph = serde_json::json!({
        "name": name,
        "nodes": spec["nodes"],
        "edges": spec["edges"],
    });

    string_to_js(&graph.to_string())
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(
    name_ptr: *const StringHeader,
    spec_json: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_str = string_from_header(spec_json).unwrap_or_default();
    let mut spec: serde_json::Value = serde_json::from_str(&spec_str).unwrap_or_default();

    spec["name"] = serde_json::Value::String(name.clone());
    spec["id"] = serde_json::Value::String(name);

    string_to_js(&spec.to_string())
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(
    graph_json: *const StringHeader,
    opts_json: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let graph = match parse_workload_graph_json(graph_json) {
        Ok(g) => g,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    let opts_str = string_from_header(opts_json).unwrap_or_default();
    let opts: perry_container_compose::types::RunGraphOptions = serde_json::from_str(&opts_str).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await;
        let project_name = format!("workload-{}", graph.name);
        let engine =
            perry_container_compose::compose::WorkloadGraphEngine::new(project_name, backend);
        match engine.run(graph, opts).await {
            Ok(handle) => {
                Ok(handle.id)
            }
            Err(e) => Err(compose_error_to_js(&e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph = match parse_workload_graph_json(graph_json) {
        Ok(g) => g,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let mut nodes = HashMap::new();
        for id in graph.nodes.keys() {
            nodes.insert(id.clone(), "pending".to_string());
        }
        let status = serde_json::json!({
            "nodes": nodes,
            "healthy": false,
            "errors": {},
        });
        Ok(types::register_string(status.to_string()))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: u64, opts_json: *const StringHeader) -> *mut Promise {
    // Shared with compose_down
    js_container_compose_down(handle_id, 0.0) // simplified
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            let mut nodes = HashMap::new();
            let mut healthy = true;
            for (id, svc) in &engine.spec.services {
                let container_name = perry_container_compose::service::Service::generate_name(
                    svc.image.as_deref().unwrap_or("unknown"), id);
                let state = if let Ok(info) = engine.backend.inspect(&container_name).await {
                    if info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up") {
                        "running".to_string()
                    } else {
                        healthy = false;
                        "stopped".to_string()
                    }
                } else {
                    healthy = false;
                    "unknown".to_string()
                };
                nodes.insert(id.clone(), state);
            }
            let status = serde_json::json!({
                "nodes": nodes,
                "healthy": healthy,
                "errors": {},
            });
            Ok(types::register_string(status.to_string()))
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    handle_id: u64,
    node_ptr: *const StringHeader,
    opts_json: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let node = string_from_header(node_ptr).unwrap_or_default();
    let opts_str = string_from_header(opts_json).unwrap_or_default();
    let opts: serde_json::Value = serde_json::from_str(&opts_str).unwrap_or_default();
    let tail = opts["tail"].as_u64().map(|t| t as u32);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.logs(Some(&node), tail).await {
                Ok(logs) => {
                    Ok(types::register_container_logs(logs.into()))
                }
                Err(e) => Err(compose_error_to_js(&e)),
            }
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    handle_id: u64,
    node_ptr: *const StringHeader,
    cmd_json: *const StringHeader,
) -> *mut Promise {
    js_container_compose_exec(handle_id, node_ptr, cmd_json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.ps().await {
                Ok(list) => {
                    let infos: Vec<serde_json::Value> = list.into_iter().map(|i| {
                        serde_json::json!({
                            "nodeId": i.name.clone(),
                            "name": i.name,
                            "containerId": Some(i.id),
                            "state": if i.status.contains("running") { "running" } else { "stopped" },
                            "image": Some(i.image),
                        })
                    }).collect();
                    Ok(types::register_string(serde_json::to_string(&infos).unwrap()))
                }
                Err(e) => Err(compose_error_to_js(&e)),
            }
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: u64) -> *const StringHeader {
    if let Some(engine) = types::get_compose_engine(handle_id) {
        let json = serde_json::to_string(&engine.spec).unwrap_or_default();
        return string_to_js(&json);
    }
    string_to_js("{}")
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // Backend is lazily initialized on first use.
}
