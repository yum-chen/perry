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

// Re-export commonly used types
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::sync::Arc;
use std::collections::HashMap;
use context::{ContainerContext, HandleEntry};

/// Get or initialize the global backend instance
async fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    ContainerContext::global().get_backend().await
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.list(all != 0).await {
            Ok(containers) => {
                let handle_id = types::register_container_info_list(containers);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.inspect(&id).await {
            Ok(info) => {
                let handle_id = types::register_container_info(info);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get the current backend name
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Some(b) = ContainerContext::global().backend.get() {
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
        match detect_backend().await {
            Ok(b) => {
                let name = b.backend_name().to_string();
                let json = serde_json::json!([{
                    "name": name,
                    "available": true,
                    "reason": ""
                }]).to_string();
                Ok(json)
            }
            Err(e) => {
                let json = if let perry_container_compose::error::ComposeError::NoBackendFound { probed } = e {
                    serde_json::to_string(&probed).unwrap_or_default()
                } else {
                    "[]".to_string()
                };
                Ok(json) // Resolve with probe info array on failure to find any
            }
        }
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

    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.list_images().await {
            Ok(images) => {
                let handle_id = types::register_image_info_list(images);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
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
/// FFI: js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
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
        match compose::compose_up(spec).await {
            Ok(handle) => Ok(handle),
            Err(e) => Err::<ComposeHandle, String>(e),
        }
    }, |handle| {
        perry_runtime::JSValue::number(handle.stack_id as f64).bits()
    });

    promise
}

/// Stop and remove compose stack.
/// FFI: js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise
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

/// Get container info for compose stack
/// FFI: js_container_compose_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_ps(handle_id as u64).await {
            Ok(containers) => {
                let h = types::register_container_info_list(containers);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_container_compose_logs(handle_id: i64, service: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *mut Promise {
    let promise = js_promise_new();

    let service = unsafe { string_from_header(service_ptr) };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_logs(handle_id as u64, service, tail_opt).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Execute command in compose service
/// FFI: js_container_compose_exec(handle_id: i64, service: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let service_opt = unsafe { string_from_header(service_ptr) };
    let cmd_json = unsafe { string_from_header(cmd_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err::<u64, String>("Invalid service name".to_string()),
        };

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_exec(handle_id as u64, service, cmd).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get resolved configuration for compose stack
/// FFI: js_container_compose_config(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_config(handle_id as u64).await
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Start services in compose stack
/// FFI: js_container_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_start(handle_id as u64, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop services in compose stack
/// FFI: js_container_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_stop(handle_id as u64, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Restart services in compose stack
/// FFI: js_container_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_restart(handle_id as u64, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Workload Graph Functions ============

/// Create a workload graph
/// FFI: js_workload_graph(name: *const StringHeader, spec_json: *const StringHeader) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(
    name_ptr: *const StringHeader,
    spec_json_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_default();

    // In a full implementation, we'd parse and build the graph here.
    // For now, we return the JSON representation as the "graph object".
    let graph = workload::WorkloadGraph {
        name: name.clone(),
        nodes: serde_json::from_str(&spec_json).unwrap_or_default(),
    };

    let result_json = serde_json::to_string(&graph).unwrap_or_default();
    string_to_js(&result_json)
}

/// Create a workload node
/// FFI: js_workload_node(name: *const StringHeader, spec_json: *const StringHeader) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_node(
    name_ptr: *const StringHeader,
    spec_json_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_default();

    let mut node: workload::WorkloadNode = serde_json::from_str(&spec_json).unwrap_or_else(|_| workload::WorkloadNode {
        id: name.clone(),
        name: name.clone(),
        image: None,
        resources: None,
        ports: vec![],
        env: HashMap::new(),
        depends_on: vec![],
        runtime: workload::RuntimeSpec::Auto,
        policy: workload::PolicySpec {
            tier: workload::PolicyTier::Default,
            no_network: false,
            read_only_root: false,
            seccomp: false,
        },
    });
    node.name = name;

    let result_json = serde_json::to_string(&node).unwrap_or_default();
    string_to_js(&result_json)
}

/// Run a workload graph
/// FFI: js_workload_runGraph(graph_json: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(
    graph_json_ptr: *const StringHeader,
    opts_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let graph_json = string_from_header(graph_json_ptr);
    let opts_json = string_from_header(opts_json_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let graph: workload::WorkloadGraph = graph_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or_else(|| "Invalid graph JSON".to_string())?;

        let _opts: workload::RunGraphOptions = opts_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(workload::RunGraphOptions { strategy: None, on_failure: None });

        // Mapping WorkloadGraph to ComposeSpec for execution
        let mut services = indexmap::IndexMap::new();
        for (id, node) in &graph.nodes {
            services.insert(id.clone(), perry_container_compose::types::ComposeService {
                image: node.image.clone(),
                ports: Some(node.ports.iter().map(|p| perry_container_compose::types::PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect()),
                environment: Some(perry_container_compose::types::ListOrDict::Dict(
                    node.env.iter().map(|(k, v)| {
                        let val = match v {
                            workload::WorkloadEnvValue::Literal(s) => Some(serde_yaml::Value::String(s.clone())),
                            workload::WorkloadEnvValue::Ref(r) => Some(serde_yaml::Value::String(format!("ref:{}", r.node_id))),
                        };
                        (k.clone(), val)
                    }).collect()
                )),
                depends_on: if node.depends_on.is_empty() { None } else {
                    Some(perry_container_compose::types::DependsOnSpec::List(node.depends_on.clone()))
                },
                ..Default::default()
            });
        }

        let spec = perry_container_compose::types::ComposeSpec {
            name: Some(graph.name.clone()),
            services,
            ..Default::default()
        };

        match compose::compose_up(spec).await {
            Ok(handle) => {
                let graph_handle = workload::GraphHandle {
                    id: handle.stack_id,
                    graph,
                };
                Ok(graph_handle)
            },
            Err(e) => Err::<workload::GraphHandle, String>(e),
        }
    }, |handle| {
        let id = ContainerContext::global().register_handle(HandleEntry::Graph(handle));
        perry_runtime::JSValue::number(id as f64).bits()
    });

    promise
}

/// Get status of a workload graph
/// FFI: js_workload_handle_status(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = perry_container_compose::ComposeEngine::get_engine(handle_id as u64)
            .ok_or_else(|| format!("Graph handle {} not found", handle_id))?;

        let status = engine.status().await.map_err(|e| e.to_string())?;

        let mut nodes = HashMap::new();
        for svc in status.services {
            let state = match svc.state.as_str() {
                "running" | "Up" => workload::NodeState::Running,
                "stopped" | "Exited" => workload::NodeState::Stopped,
                _ => workload::NodeState::Unknown,
            };
            nodes.insert(svc.service, state);
        }

        Ok(workload::GraphStatus {
            nodes,
            healthy: status.healthy,
            errors: HashMap::new(),
        })
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Stop and remove workload graph
/// FFI: js_workload_handle_down(handle_id: i64, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: i64, _opts_ptr: *const StringHeader) -> *mut Promise {
    // Reuse compose_down
    js_container_compose_down(handle_id, 0)
}

/// Get logs from workload graph
/// FFI: js_workload_handle_logs(handle_id: i64, node: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    handle_id: i64,
    node_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    let tail = string_from_header(opts_ptr)
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("tail").and_then(|t| t.as_u64()).map(|t| t as i32))
        .unwrap_or(-1);
    js_container_compose_logs(handle_id, node_ptr, tail)
}

/// Execute command in workload graph
/// FFI: js_workload_handle_exec(handle_id: i64, node: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    handle_id: i64,
    node_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_exec(handle_id, node_ptr, cmd_ptr)
}

/// Get node info for workload graph
/// FFI: js_workload_handle_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = perry_container_compose::ComposeEngine::get_engine(handle_id as u64)
            .ok_or_else(|| format!("Graph handle {} not found", handle_id))?;

        let infos = engine.ps().await.map_err(|e| e.to_string())?;

        let node_infos: Vec<workload::NodeInfo> = infos.into_iter().map(|i| {
            let state = match i.status.as_str() {
                "running" | "Up" => workload::NodeState::Running,
                "stopped" | "Exited" => workload::NodeState::Stopped,
                _ => workload::NodeState::Unknown,
            };
            workload::NodeInfo {
                node_id: i.labels.get("com.docker.compose.service").cloned().unwrap_or_default(),
                name: i.name,
                container_id: Some(i.id),
                state,
                image: Some(i.image),
            }
        }).collect();

        Ok(node_infos)
    }, |infos| {
        let json = serde_json::to_string(&infos).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

/// Get the workload graph from a handle
/// FFI: js_workload_handle_graph(handle_id: i64) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: i64) -> *const StringHeader {
    if let Some(entry) = ContainerContext::global().get_handle(handle_id as u64) {
        if let HandleEntry::Graph(gh) = &*entry {
            let json = serde_json::to_string(&gh.graph).unwrap_or_default();
            return string_to_js(&json);
        }
    }
    string_to_js("{}")
}

/// Get the resolved graph for a compose stack
/// FFI: js_container_compose_graph(handle_id: i64) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(handle_id: i64) -> *const StringHeader {
    if let Some(engine) = perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        if let Ok(graph) = engine.graph() {
            let json = serde_json::to_string(&graph).unwrap_or_default();
            return string_to_js(&json);
        }
    }
    string_to_js("{}")
}

/// Get the status of a compose stack
/// FFI: js_container_compose_status(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = perry_container_compose::ComposeEngine::get_engine(handle_id as u64)
            .ok_or_else(|| format!("Compose handle {} not found", handle_id))?;
        engine.status().await.map_err(|e| e.to_string())
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

/// Inspect a workload graph without starting it
/// FFI: js_workload_inspectGraph(graph_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_json_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let graph: workload::WorkloadGraph = graph_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or_else(|| "Invalid graph JSON".to_string())?;

        let mut nodes = HashMap::new();
        for id in graph.nodes.keys() {
            nodes.insert(id.clone(), workload::NodeState::Pending);
        }

        Ok(workload::GraphStatus {
            nodes,
            healthy: false,
            errors: HashMap::new(),
        })
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // Optionally trigger background backend detection
    tokio::spawn(async {
        let _ = get_global_backend().await;
    });
}
