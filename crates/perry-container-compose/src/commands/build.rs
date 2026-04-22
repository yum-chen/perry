use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct BuildCommand<'a> {
    pub service: &'a Service,
}

#[async_trait]
impl<'a> ContainerCommand for BuildCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        self.service.build_command(backend).await
    }
}
