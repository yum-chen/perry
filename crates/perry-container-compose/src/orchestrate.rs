use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use crate::commands::start::StartCommand;
use crate::commands::run::RunCommand;

pub async fn orchestrate_service(service: &Service, backend: &dyn ContainerBackend) -> Result<()> {
    if service.is_running(backend).await? {
        tracing::info!(service = %service.name, "already running, skipping");
        return Ok(());
    }

    if service.exists(backend).await? {
        tracing::info!(service = %service.name, "exists but stopped, starting");
        let cmd = StartCommand { service };
        cmd.exec(backend).await?;
    } else {
        tracing::info!(service = %service.name, "creating and running");
        let cmd = RunCommand { service };
        cmd.exec(backend).await?;
    }

    Ok(())
}
