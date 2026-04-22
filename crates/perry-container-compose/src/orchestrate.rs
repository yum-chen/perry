use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::ComposeService;

pub async fn orchestrate_service(
    svc_name: &str,
    service: &ComposeService,
    backend: &dyn ContainerBackend,
) -> Result<()> {
    if service.is_running(svc_name, backend).await {
        tracing::info!(service = svc_name, "already running, skipping");
        return Ok(());
    }

    if service.exists(svc_name, backend).await {
        tracing::info!(service = svc_name, "exists but stopped, starting");
        let cmd = crate::commands::StartCommand { service_name: svc_name, service, backend };
        cmd.exec().await
    } else {
        if service.needs_build() {
            tracing::info!(service = svc_name, "building image");
            let cmd = crate::commands::BuildCommand { service_name: svc_name, service, backend };
            cmd.exec().await?;
        }

        tracing::info!(service = svc_name, "creating and running");
        let cmd = crate::commands::RunCommand { service_name: svc_name, service, backend };
        cmd.exec().await.map(|_| ())
    }
}
