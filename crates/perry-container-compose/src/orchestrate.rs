//! Service orchestration logic.

use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::ComposeService;

/// Orchestrate a single service startup.
///
/// Logic:
/// 1. If running -> skip
/// 2. If exists but stopped -> start_command
/// 3. If not exists -> (build if needed) -> run_command
pub async fn orchestrate_service(
    service: &ComposeService,
    service_name: &str,
    backend: &dyn ContainerBackend,
) -> Result<()> {
    if service.is_running(backend, service_name).await? {
        tracing::info!(service = %service_name, "already running, skipping");
        return Ok(());
    }

    if service.exists(backend, service_name).await? {
        tracing::info!(service = %service_name, "exists but stopped, starting");
        service.start_command(backend, service_name).await?;
    } else {
        if service.needs_build() {
            tracing::info!(service = %service_name, "building image");
            service.build_command(backend, service_name).await?;
        }
        tracing::info!(service = %service_name, "creating and running");
        service.run_command(backend, service_name).await?;
    }

    Ok(())
}
