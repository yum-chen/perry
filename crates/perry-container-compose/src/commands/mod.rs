use crate::error::{ComposeError, Result};
use crate::backend::ContainerBackend;
use crate::types::{ContainerHandle, ContainerSpec, ComposeService};
use std::sync::Arc;
use indexmap::IndexMap;
use std::collections::HashMap;

pub struct BuildCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a ComposeService,
    pub backend: &'a dyn ContainerBackend,
}

impl<'a> BuildCommand<'a> {
    pub async fn exec(&self) -> Result<()> {
        self.service.build_command(self.service_name, self.backend).await
    }
}

pub struct RunCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a ComposeService,
    pub backend: &'a dyn ContainerBackend,
}

impl<'a> RunCommand<'a> {
    pub async fn exec(&self) -> Result<ContainerHandle> {
        let container_name = crate::service::service_container_name(self.service, self.service_name);
        let spec = ContainerSpec {
            image: self.service.image_ref(self.service_name),
            name: Some(container_name),
            ports: Some(self.service.port_strings()),
            volumes: Some(self.service.volume_strings()),
            env: Some(self.service.resolved_env()),
            cmd: self.service.command_list(),
            rm: Some(false),
            ..Default::default()
        };
        self.backend.run(&spec).await
    }
}

pub struct StartCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a ComposeService,
    pub backend: &'a dyn ContainerBackend,
}

impl<'a> StartCommand<'a> {
    pub async fn exec(&self) -> Result<()> {
        let container_name = crate::service::service_container_name(self.service, self.service_name);
        self.backend.start(&container_name).await
    }
}

pub struct StopCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a ComposeService,
    pub backend: &'a dyn ContainerBackend,
}

impl<'a> StopCommand<'a> {
    pub async fn exec(&self) -> Result<()> {
        let container_name = crate::service::service_container_name(self.service, self.service_name);
        self.backend.stop(&container_name, None).await
    }
}

pub struct InspectCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a ComposeService,
    pub backend: &'a dyn ContainerBackend,
}

impl<'a> InspectCommand<'a> {
    pub async fn exec(&self) -> Result<crate::types::ContainerInfo> {
        let container_name = crate::service::service_container_name(self.service, self.service_name);
        self.backend.inspect(&container_name).await
    }
}
