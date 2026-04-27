//! Container module for Perry

pub mod backend;
pub mod compose;
pub mod types;
pub mod verification;
pub mod capability;

pub use types::{
    ComposeHealthcheck, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume,
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeError,
    ContainerError, string_from_header
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, atomic::{AtomicU64, Ordering}};
use dashmap::DashMap;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::workload::{WorkloadGraphEngine, WorkloadGraph, RunOptions, GraphHandle};
use crate::container::backend::get_global_backend_instance;

static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
static COMPOSE_HANDLES: OnceLock<DashMap<u64, ComposeEngine>> = OnceLock::new();
static WORKLOAD_HANDLES: OnceLock<DashMap<u64, GraphHandle>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

fn get_next_id() -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Ok(rt) = tokio::runtime::Handle::try_current() {
        if let Ok(backend) = rt.block_on(get_global_backend_instance()) {
            let name = backend.backend_name();
            return perry_runtime::js_string_from_bytes(name.as_ptr(), name.len() as u32);
        }
    }
    std::ptr::null()
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let res = perry_container_compose::backend::detect_backend().await;
        match res {
            Ok(backend) => {
                let info = serde_json::json!([{
                    "name": backend.backend_name(),
                    "available": true,
                }]);
                let s = info.to_string();
                Ok(perry_runtime::js_string_from_bytes(s.as_ptr(), s.len() as u32) as u64)
            }
            Err(probed) => {
                let info = serde_json::to_string(&probed).unwrap_or_default();
                Ok(perry_runtime::js_string_from_bytes(info.as_ptr(), info.len() as u32) as u64)
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: f64, _opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match WORKLOAD_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Workload handle not found".into()) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        for name in handle.graph.nodes.keys() {
            let _ = backend.stop(name, None).await;
            let _ = backend.remove(name, true).await;
        }
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(handle_id: f64, node_name_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let node_name = match string_from_header(node_name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid node name".into()) });
            return promise;
        }
    };
    let tail = string_from_header(opts_json_ptr).and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["tail"].as_u64().map(|n| n as u32));

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = backend.logs(&node_name, tail).await.map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&logs).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(handle_id: f64, node_name_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let node_name = match string_from_header(node_name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid node name".into()) });
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid cmd JSON".into()) });
            return promise;
        }
    };
    let cmd: Vec<String> = match serde_json::from_str(&cmd_json) {
        Ok(c) => c,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = backend.exec(&node_name, &cmd, None, None).await.map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&logs).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match WORKLOAD_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Workload handle not found".into()) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let mut ps = Vec::new();
        for name in handle.graph.nodes.keys() {
             if let Ok(info) = backend.inspect(name).await {
                 ps.push(info);
             }
        }
        let json = serde_json::to_string(&ps).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = match string_from_header(graph_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid graph JSON".into()) });
            return promise;
        }
    };
    let opts_json = match string_from_header(opts_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid opts JSON".into()) });
            return promise;
        }
    };

    let graph: WorkloadGraph = match serde_json::from_str(&graph_json) {
        Ok(g) => g,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(msg) });
            return promise;
        }
    };
    let opts: RunOptions = match serde_json::from_str(&opts_json) {
        Ok(o) => o,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(msg) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let engine = WorkloadGraphEngine::new(backend);
        let handle = engine.run(&graph, &opts).await.map_err(|e| e.to_string())?;
        let id = handle.id;
        WORKLOAD_HANDLES.get_or_init(DashMap::new).insert(id, handle);
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match WORKLOAD_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Workload handle not found".into()) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let mut statuses = HashMap::new();
        for name in handle.graph.nodes.keys() {
             let info = backend.inspect(name).await.map(|i| i.status).unwrap_or_else(|_| "unknown".into());
             statuses.insert(name.clone(), info);
        }
        let json = serde_json::to_string(&statuses).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_build(spec_json_ptr: *const StringHeader, image_name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid spec JSON".into()) });
            return promise;
        }
    };
    let image_name = match string_from_header(image_name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid image name".into()) });
            return promise;
        }
    };

    let spec: perry_container_compose::types::ComposeServiceBuild = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.build(&spec, &image_name).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ContainerSpec: {}", msg))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let handle = backend.run(&perry_container_compose::types::ContainerSpec {
            image: spec.image,
            name: spec.name,
            ports: spec.ports,
            volumes: spec.volumes,
            env: spec.env,
            cmd: spec.cmd,
            entrypoint: spec.entrypoint,
            network: spec.network,
            rm: spec.rm,
            read_only: None,
        }).await.map_err(|e| e.to_string())?;

        let id = get_next_id();
        let handle_mapped = ContainerHandle { id: handle.id, name: handle.name };
        CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle_mapped);
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ContainerSpec: {}", msg))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let handle = backend.create(&perry_container_compose::types::ContainerSpec {
            image: spec.image,
            name: spec.name,
            ports: spec.ports,
            volumes: spec.volumes,
            env: spec.env,
            cmd: spec.cmd,
            entrypoint: spec.entrypoint,
            network: spec.network,
            rm: spec.rm,
            read_only: None,
        }).await.map_err(|e| e.to_string())?;

        let id = get_next_id();
        let handle_mapped = ContainerHandle { id: handle.id, name: handle.name };
        CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle_mapped);
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.start(&id).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.stop(&id, if timeout >= 0.0 { Some(timeout as u32) } else { None }).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove(&id, force != 0.0).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let list = backend.list(all != 0.0).await.map_err(|e| e.to_string())?;
        let mapped: Vec<ContainerInfo> = list.into_iter().map(|i| ContainerInfo {
            id: i.id, name: i.name, image: i.image, status: i.status, ports: i.ports, created: i.created
        }).collect();
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let info = backend.inspect(&id).await.map_err(|e| e.to_string())?;
        let mapped = ContainerInfo {
            id: info.id, name: info.name, image: info.image, status: info.status, ports: info.ports, created: info.created
        };
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = backend.logs(&id, if tail >= 0.0 { Some(tail as u32) } else { None }).await.map_err(|e| e.to_string())?;
        let mapped = ContainerLogs { stdout: logs.stdout, stderr: logs.stderr };
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(id_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader, env_json_ptr: *const StringHeader, workdir_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".to_string())
            });
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid cmd JSON".to_string())
            });
            return promise;
        }
    };
    let cmd: Vec<String> = match serde_json::from_str(&cmd_json) {
        Ok(c) => c,
        Err(_) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid cmd format".to_string())
            });
            return promise;
        }
    };

    let env_json = string_from_header(env_json_ptr);
    let env: Option<HashMap<String, String>> = env_json.and_then(|s| serde_json::from_str(&s).ok());
    let workdir = string_from_header(workdir_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| e.to_string())?;
        let mapped = ContainerLogs { stdout: logs.stdout, stderr: logs.stderr };
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid Reference".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let list = backend.list_images().await.map_err(|e| e.to_string())?;
        let mapped: Vec<ImageInfo> = list.into_iter().map(|i| ImageInfo {
            id: i.id, repository: i.repository, tag: i.tag, size: i.size, created: i.created
        }).collect();
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid Reference".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove_image(&reference, force != 0.0).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ComposeSpec: {}", msg))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let engine = ComposeEngine::new(spec, backend);
        let handle = engine.up().await.map_err(|e| e.to_string())?;

        let id = handle.stack_id;
        COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, engine);
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: f64, volumes: f64) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match COMPOSE_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(e) => e.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Compose handle not found".into())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        engine.down(volumes != 0.0).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match COMPOSE_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(e) => e.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Compose handle not found".into())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ps = engine.ps().await.map_err(|e| e.to_string())?;
        let mapped: Vec<ContainerInfo> = ps.into_iter().map(|i| ContainerInfo {
            id: i.id, name: i.name, image: i.image, status: i.status, ports: i.ports, created: i.created
        }).collect();
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(handle_id: f64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match COMPOSE_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(e) => e.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Compose handle not found".into())
            });
            return promise;
        }
    };
    let service = string_from_header(service_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let logs = engine.logs(service.as_deref(), if tail >= 0.0 { Some(tail as u32) } else { None }).await.map_err(|e| e.to_string())?;
        let mapped = ContainerLogs { stdout: logs.stdout, stderr: logs.stderr };
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(handle_id: f64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader, _opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match COMPOSE_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(e) => e.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Compose handle not found".into())
            });
            return promise;
        }
    };
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid service name".into())
            });
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid cmd JSON".into())
            });
            return promise;
        }
    };
    let cmd: Vec<String> = match serde_json::from_str(&cmd_json) {
        Ok(c) => c,
        Err(_) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid cmd format".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let logs = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
        let mapped = ContainerLogs { stdout: logs.stdout, stderr: logs.stderr };
        let json = serde_json::to_string(&mapped).unwrap_or_default();
        Ok(perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32) as u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_id: f64, _services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match COMPOSE_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(e) => e.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Compose handle not found".into())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        engine.start(&[]).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_id: f64, _services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match COMPOSE_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(e) => e.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Compose handle not found".into())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        engine.stop(&[]).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_id: f64, _services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match COMPOSE_HANDLES.get_or_init(DashMap::new).get(&(handle_id as u64)) {
        Some(e) => e.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Compose handle not found".into())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        engine.restart(&[]).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        // Just echo the spec as "config" for now
        Ok(perry_runtime::js_string_from_bytes(spec_json.as_ptr(), spec_json.len() as u32) as u64)
    });
    promise
}
