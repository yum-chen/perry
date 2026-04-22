//! Compose orchestration shim.

use crate::container::context::{ContainerContext, HandleEntry};
use crate::container::types::{ContainerInfo, ContainerLogs};
use perry_container_compose::types::{ComposeHandle, ComposeSpec};
use perry_container_compose::ComposeEngine;
use std::sync::Arc;

pub async fn compose_up(spec: ComposeSpec) -> Result<ComposeHandle, String> {
    let ctx = ContainerContext::global();
    let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;
    let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
    let engine = Arc::new(ComposeEngine::new(spec, project_name, backend));

    let handle = engine.up(&[], true, false, false).await.map_err(|e| e.to_string())?;

    // We store the engine so we can perform operations on it via the handle's ID
    ctx.handles.insert(handle.stack_id, HandleEntry::Engine(engine));

    Ok(handle)
}

pub async fn compose_down(id: u64, volumes: bool) -> Result<(), String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    engine.down(&[], false, volumes).await.map_err(|e| e.to_string())?;
    ctx.handles.remove(&id);
    Ok(())
}

pub async fn compose_ps(id: u64) -> Result<Vec<ContainerInfo>, String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    let infos = engine.ps().await.map_err(|e| e.to_string())?;
    Ok(infos.into_iter().map(|i| ContainerInfo {
        id: i.id,
        name: i.name,
        image: i.image,
        status: i.status,
        ports: i.ports,
        labels: i.labels,
        created: i.created,
    }).collect())
}

pub async fn compose_logs(id: u64, service: Option<String>, tail: Option<u32>) -> Result<ContainerLogs, String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    let services = service.map(|s| vec![s]).unwrap_or_default();
    let logs_map = engine.logs(&services, tail).await.map_err(|e| e.to_string())?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    for (svc, logs) in logs_map {
        stdout.push_str(&format!("[{}] {}\n", svc, logs.stdout));
        stderr.push_str(&format!("[{}] {}\n", svc, logs.stderr));
    }

    Ok(ContainerLogs { stdout, stderr })
}

pub async fn compose_exec(id: u64, service: String, cmd: Vec<String>) -> Result<ContainerLogs, String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    let logs = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
    Ok(ContainerLogs {
        stdout: logs.stdout,
        stderr: logs.stderr,
    })
}

pub async fn compose_config(id: u64) -> Result<String, String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    engine.config().map_err(|e| e.to_string())
}

pub async fn compose_start(id: u64, services: Vec<String>) -> Result<(), String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    engine.start(&services).await.map_err(|e| e.to_string())
}

pub async fn compose_stop(id: u64, services: Vec<String>) -> Result<(), String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    engine.stop(&services).await.map_err(|e| e.to_string())
}

pub async fn compose_restart(id: u64, services: Vec<String>) -> Result<(), String> {
    let ctx = ContainerContext::global();
    let engine = match ctx.handles.get(&id) {
        Some(entry) => match &*entry {
            HandleEntry::Engine(e) => Arc::clone(e),
            _ => return Err(format!("Handle {} is not a compose engine", id)),
        },
        None => return Err(format!("Compose stack {} not found", id)),
    };

    engine.restart(&services).await.map_err(|e| e.to_string())
}
