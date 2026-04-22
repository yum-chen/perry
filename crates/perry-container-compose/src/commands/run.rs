use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct RunCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a Service,
}

#[async_trait]
impl<'a> ContainerCommand for RunCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        self.service.run_command(self.service_name, backend).await
    }
}
