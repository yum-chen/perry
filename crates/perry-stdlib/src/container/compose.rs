//! Compose orchestration wrapper.

use super::types::{ContainerInfo, ContainerLogs};
use perry_container_compose::types::{ComposeHandle, ComposeSpec};
use perry_container_compose::ComposeEngine;
use std::sync::Arc;
use crate::container::context::{ContainerContext, HandleEntry};

pub async fn compose_up(spec: ComposeSpec) -> Result<ComposeHandle, String> {
    let backend = ContainerContext::global().get_backend().await.map_err(|e| e.to_string())?;
    let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
    let engine = ComposeEngine::new(spec, project_name, backend);

    let handle = engine.up(&[], true, false, false).await.map_err(|e| e.to_string())?;

    // We need to store the engine in the context to perform operations on the handle later
    // For simplicity, we register a Compose handle entry.
    // In a full implementation, we might need to store the Engine itself.
    ContainerContext::global().register_handle(HandleEntry::Compose(handle.clone()));

    Ok(handle)
}

pub async fn compose_down(id: u64, volumes: bool) -> Result<(), String> {
    let _handle = match ContainerContext::global().take_handle(id) {
        Some(HandleEntry::Compose(h)) => h,
        _ => return Err(format!("Compose stack {} not found", id)),
    };

    // In this simplified wrapper, we don't store the engine,
    // but the actual ComposeEngine in perry-container-compose
    // has a static COMPOSE_ENGINES registry.
    if let Some(engine) = ComposeEngine::get_engine(id) {
        engine.down(&[], false, volumes).await.map_err(|e| e.to_string())?;
        ComposeEngine::unregister(id);
        Ok(())
    } else {
        Err(format!("Compose engine for stack {} not found", id))
    }
}

pub async fn compose_ps(id: u64) -> Result<Vec<ContainerInfo>, String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

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
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    let services = service.map(|s| vec![s]).unwrap_or_default();
    let logs = engine.logs(&services, tail).await.map_err(|e| e.to_string())?;

    Ok(ContainerLogs { stdout: logs.stdout, stderr: logs.stderr })
}

pub async fn compose_exec(id: u64, service: String, cmd: Vec<String>) -> Result<ContainerLogs, String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    let logs = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
    Ok(ContainerLogs {
        stdout: logs.stdout,
        stderr: logs.stderr,
    })
}

pub async fn compose_config(id: u64) -> Result<String, String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.config().map_err(|e| e.to_string())
}

pub async fn compose_start(id: u64, services: Vec<String>) -> Result<(), String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.start(&services).await.map_err(|e| e.to_string())
}

pub async fn compose_stop(id: u64, services: Vec<String>) -> Result<(), String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.stop(&services).await.map_err(|e| e.to_string())
}

pub async fn compose_restart(id: u64, services: Vec<String>) -> Result<(), String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.restart(&services).await.map_err(|e| e.to_string())
}
