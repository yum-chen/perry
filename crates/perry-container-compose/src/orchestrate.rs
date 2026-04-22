use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::build::BuildCommand;
use crate::commands::run::RunCommand;
use crate::commands::start::StartCommand;
use crate::commands::ContainerCommand;

pub async fn orchestrate_service(service: &Service<'_>, backend: &dyn ContainerBackend) -> Result<()> {
    if service.is_running(backend).await? {
        tracing::info!(service = %service.name, "already running, skipping");
        return Ok(());
    }

    if service.exists(backend).await? {
        tracing::info!(service = %service.name, "exists but stopped, starting");
        StartCommand { service }.exec(backend).await?;
    } else {
        if service.needs_build() {
            tracing::info!(service = %service.name, "building image");
            BuildCommand { service }.exec(backend).await?;
        }
        tracing::info!(service = %service.name, "creating and running");
        RunCommand { service }.exec(backend).await?;
    }
    Ok(())
}

pub async fn stop_service(service: &Service<'_>, backend: &dyn ContainerBackend) -> Result<()> {
    if service.is_running(backend).await? {
        tracing::info!(service = %service.name, "stopping service");
        backend.stop(&service.container_name(), None).await?;
    }
    Ok(())
}

pub async fn remove_service(service: &Service<'_>, backend: &dyn ContainerBackend) -> Result<()> {
    if service.exists(backend).await? {
        tracing::info!(service = %service.name, "removing service");
        backend.remove(&service.container_name(), true).await?;
    }
    Ok(())
}
