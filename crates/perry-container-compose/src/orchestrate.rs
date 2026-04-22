use crate::error::{ComposeError, Result};
use crate::backend::ContainerBackend;
use crate::service::Service;
// Note: commands are assumed to be implemented in crates/perry-container-compose/src/commands/
// For this plan step, we focus on the orchestrate_service logic.

/// Requirement 19.1, 19.2: Core per-service startup function ported from cmd/start/cmd.go
pub async fn orchestrate_service(
    service_name: &str,
    service: &Service,
    backend: &dyn ContainerBackend
) -> Result<()> {
    if service.is_running(service_name, backend).await? {
        tracing::info!(service = %service_name, "already running, skipping");
        return Ok(());
    }

    if service.exists(service_name, backend).await? {
        tracing::info!(service = %service_name, "exists but stopped, starting");
        service.start_command(service_name, backend).await?;
    } else {
        if service.needs_build() {
            tracing::info!(service = %service_name, "building image");
            service.build_command(service_name, backend).await?;
        }
        tracing::info!(service = %service_name, "creating and running");
        service.run_command(service_name, backend).await?;
    }
    Ok(())
}

pub async fn stop_service(
    service_name: &str,
    service: &Service,
    backend: &dyn ContainerBackend
) -> Result<()> {
    if service.is_running(service_name, backend).await? {
        tracing::info!(service = %service_name, "stopping service");
        backend.stop(&service.container_name(service_name), None).await?;
    }
    Ok(())
}

pub async fn remove_service(
    service_name: &str,
    service: &Service,
    backend: &dyn ContainerBackend
) -> Result<()> {
    if service.exists(service_name, backend).await? {
        tracing::info!(service = %service_name, "removing service");
        backend.remove(&service.container_name(service_name), true).await?;
    }
    Ok(())
}
