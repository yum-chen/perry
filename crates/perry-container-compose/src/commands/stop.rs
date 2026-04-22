use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct StopCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a Service,
}

#[async_trait]
impl<'a> ContainerCommand for StopCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.stop(&self.service.container_name(self.service_name), None).await
    }
}
