//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod context;
pub mod types;
pub mod verification;
pub mod workload;

use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::sync::Arc;
use self::context::ContainerContext;
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
        let backend = ContainerContext::global().get_backend().await?;
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    handle_id: u64,
    node_ptr: *const StringHeader,
    tail: f64,
) -> *mut Promise {
    js_container_compose_logs(handle_id, node_ptr, tail)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    handle_id: u64,
    node_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_exec(handle_id, node_ptr, cmd_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: u64) -> *const StringHeader {
    if let Some(h) = types::get_compose_handle(handle_id) {
        if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
            let json = serde_json::to_string(&engine.spec).unwrap_or_default();
            return string_to_js(&json);
        }
    }
    string_to_js("{}")
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(
    graph_json: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let graph_str = string_from_header(graph_json).unwrap_or_default();
    let graph: workload::WorkloadGraph = match serde_json::from_str(&graph_str) {
        Ok(g) => g,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid WorkloadGraph: {}", e))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let mut nodes = std::collections::HashMap::new();
        for id in graph.nodes.keys() {
            nodes.insert(id.clone(), workload::NodeState::Pending);
        }
        let status = workload::GraphStatus {
            nodes,
            healthy: false,
            errors: std::collections::HashMap::new(),
        };
        Ok(types::register_string(
            serde_json::to_string(&status).unwrap_or_default(),
        ))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: u64, volumes: f64) -> *mut Promise {
    js_container_compose_down(handle_id, volumes)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                match engine.status().await {
                    Ok(stack_status) => {
                        let mut nodes = std::collections::HashMap::new();
                        for svc in stack_status.services {
                            let state = match svc.state {
                                perry_container_compose::compose::ServiceState::Running => {
                                    workload::NodeState::Running
                                }
                                perry_container_compose::compose::ServiceState::Stopped => {
                                    workload::NodeState::Stopped
                                }
                                perry_container_compose::compose::ServiceState::Failed => {
                                    workload::NodeState::Failed
                                }
                                perry_container_compose::compose::ServiceState::Pending => {
                                    workload::NodeState::Pending
                                }
                                _ => workload::NodeState::Unknown,
                            };
                            nodes.insert(svc.service, state);
                        }
                        let status = workload::GraphStatus {
                            nodes,
                            healthy: stack_status.healthy,
                            errors: std::collections::HashMap::new(),
                        };
                        Ok(types::register_string(
                            serde_json::to_string(&status).unwrap_or_default(),
                        ))
                    }
                    Err(e) => Err::<u64, String>(e.to_string()),
                }
            } else {
                Err::<u64, String>("Graph engine not found".to_string())
            }
        } else {
            Err::<u64, String>("Invalid graph handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(h) = types::get_compose_handle(handle_id) {
            if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(h.stack_id) {
                match engine.status().await {
                    Ok(stack_status) => {
                        let nodes: Vec<workload::NodeInfo> = stack_status
                            .services
                            .into_iter()
                            .map(|svc| {
                                let state = match svc.state {
                                    perry_container_compose::compose::ServiceState::Running => {
                                        workload::NodeState::Running
                                    }
                                    perry_container_compose::compose::ServiceState::Stopped => {
                                        workload::NodeState::Stopped
                                    }
                                    perry_container_compose::compose::ServiceState::Failed => {
                                        workload::NodeState::Failed
                                    }
                                    perry_container_compose::compose::ServiceState::Pending => {
                                        workload::NodeState::Pending
                                    }
                                    _ => workload::NodeState::Unknown,
                                };
                                workload::NodeInfo {
                                    node_id: svc.service.clone(),
                                    name: svc.service,
                                    container_id: svc.container_id,
                                    state,
                                    image: None,
                                }
                            })
                            .collect();
                        Ok(types::register_string(
                            serde_json::to_string(&nodes).unwrap_or_default(),
                        ))
                    }
                    Err(e) => Err::<u64, String>(e.to_string()),
                }
            } else {
                Err::<u64, String>("Graph engine not found".to_string())
            }
        } else {
            Err::<u64, String>("Invalid graph handle".to_string())
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
        let backend = ContainerContext::global().get_backend().await?;
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
        ContainerContext::global().get_backend().await?
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
        ContainerContext::global().get_backend().await?
            .stop(&id, t).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        ContainerContext::global().get_backend().await?
            .remove(&id, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match ContainerContext::global().get_backend().await?.list(all != 0.0).await {
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
        match ContainerContext::global().get_backend().await?.inspect(&id).await {
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
        match ContainerContext::global().get_backend().await?.logs(&id, t).await {
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
        match ContainerContext::global().get_backend().await?
            .exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err::<u64, String>(e.to_string()),
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
    let spec: perry_container_compose::types::ComposeServiceBuild =
        match string_from_header(spec_json).and_then(|s| serde_json::from_str(&s).ok()) {
            Some(s) => s,
            None => {
                crate::common::spawn_for_promise(promise as *mut u8, async move {
                    Err::<u64, String>("Invalid build spec JSON".to_string())
                });
                return promise;
            }
        };
    let image_name = string_from_header(image_name_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        ContainerContext::global()
            .get_backend()
            .await?
            .build(&spec, &image_name)
            .await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(image_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        ContainerContext::global().get_backend().await?
            .pull_image(&image).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match ContainerContext::global().get_backend().await?.list_images().await {
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
        ContainerContext::global().get_backend().await?
            .remove_image(&image, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = if let Some(backend) = ContainerContext::global().backend.blocking_lock().as_ref() {
        backend.backend_name().to_string()
    } else {
        "unknown".to_string()
    };
    string_to_js(&name)
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
        let backend = ContainerContext::global().get_backend().await?;
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
                        Ok(types::register_container_logs(logs))
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
        match perry_container_compose::backend::detect_backend().await {
            Ok(backend) => {
                // Return detection info
                let info = types::BackendInfo {
                    name: backend.backend_name().to_string(),
                    available: true,
                    reason: None,
                    version: None,
                    mode: "local".to_string(),
                    isolation_level: backend.isolation_level(),
                };
                let json = serde_json::to_string(&vec![info]).unwrap_or_else(|_| "[]".to_string());
                Ok(types::register_string(json))
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
    crate::common::spawn(async {
        let _ = ContainerContext::global().get_backend().await;
    });
}

// ============ Workload Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(
    graph_json: *const StringHeader,
    opts_json: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let graph_str = string_from_header(graph_json).unwrap_or_default();
    let graph: workload::WorkloadGraph = match serde_json::from_str(&graph_str) {
        Ok(g) => g,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid WorkloadGraph: {}", e))
            });
            return promise;
        }
    };
    let opts_str = string_from_header(opts_json).unwrap_or_default();
    let _opts: workload::RunGraphOptions =
        serde_json::from_str(&opts_str).unwrap_or(workload::RunGraphOptions {
            strategy: None,
            on_failure: None,
        });

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = ContainerContext::global().get_backend().await?;

        // Convert WorkloadGraph to ComposeSpec
        let mut services = indexmap::IndexMap::new();
        for (id, node) in graph.nodes {
            let mut svc = types::ComposeService {
                image: node.image.clone(),
                ports: Some(
                    node.ports
                        .iter()
                        .map(|p| types::PortSpec::Short(serde_yaml::Value::String(p.clone())))
                        .collect(),
                ),
                depends_on: Some(types::DependsOnSpec::List(node.depends_on.clone())),
                ..Default::default()
            };

            let mut env = indexmap::IndexMap::new();
            for (k, v) in node.env {
                let val = match v {
                    workload::WorkloadEnvValue::Literal(s) => Some(serde_yaml::Value::String(s)),
                    workload::WorkloadEnvValue::Ref(r) => {
                        // In OCI, service names are hostnames.
                        match r.projection {
                            workload::RefProjection::Ip => Some(serde_yaml::Value::String(r.node_id)),
                            workload::RefProjection::Endpoint => {
                                let port = r.port.unwrap_or_else(|| "80".to_string());
                                Some(serde_yaml::Value::String(format!("{}:{}", r.node_id, port)))
                            }
                            workload::RefProjection::InternalUrl => {
                                Some(serde_yaml::Value::String(format!("http://{}", r.node_id)))
                            }
                        }
                    }
                };
                env.insert(k, val);
            }
            svc.environment = Some(types::ListOrDict::Dict(env));
            services.insert(id, svc);
        }

        let spec = types::ComposeSpec {
            name: Some(graph.name.clone()),
            services,
            ..Default::default()
        };

        let engine =
            Arc::new(perry_container_compose::ComposeEngine::new(spec, graph.name, backend));
        match Arc::clone(&engine).up(&[], true, true, false).await {
            Ok(handle) => Ok(types::register_compose_handle(handle)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}
