use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct StartCommand<'a> {
    pub service: &'a Service,
}

#[async_trait]
impl<'a> ContainerCommand for StartCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        self.service.start_command(backend).await
    }
}
