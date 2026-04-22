//! Compose orchestration wrapper.

use super::types::{ArcComposeEngine, ContainerInfo, ContainerLogs};
use perry_container_compose::types::{ComposeHandle, ComposeSpec};
use perry_container_compose::ComposeEngine;
use std::sync::Arc;
use crate::container::mod_private::get_global_backend_instance;
use crate::container::types::COMPOSE_HANDLES;
use dashmap::DashMap;

pub async fn compose_up(spec: ComposeSpec) -> Result<ComposeHandle, String> {
    let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
    let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
    let engine = Arc::new(ComposeEngine::new(spec, project_name, Arc::clone(&backend) as Arc<dyn perry_container_compose::ContainerBackend>));

    let handle = Arc::clone(&engine).up(&[], true, false, false).await.map_err(|e| e.to_string())?;

    Ok(handle)
}

pub async fn compose_down(id: u64, volumes: bool) -> Result<(), String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.down(&[], false, volumes).await.map_err(|e| e.to_string())?;
    ComposeEngine::unregister(id);
    Ok(())
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
        created: i.created,
    }).collect())
}

pub async fn compose_logs(id: u64, service: Option<String>, tail: Option<u32>) -> Result<ContainerLogs, String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

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

pub async fn compose_exec(id: u64, service: String, cmd: Vec<String>, env: Option<std::collections::HashMap<String, String>>, workdir: Option<String>) -> Result<ContainerLogs, String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    let svc = engine.spec.services.get(&service).ok_or_else(|| format!("Service {} not found", service))?;
    let container_name = perry_container_compose::service::service_container_name(svc, &service);

    let logs = engine.backend.exec(&container_name, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| e.to_string())?;
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
