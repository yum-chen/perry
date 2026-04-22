use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::{ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeNetwork, ComposeVolume};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub method: String,
    pub args: Vec<String>,
}

pub struct MockBackendState {
    pub calls: Vec<RecordedCall>,
}

pub struct MockBackend {
    pub state: Arc<Mutex<MockBackendState>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockBackendState {
                calls: Vec::new(),
            })),
        }
    }

    pub fn calls(&self) -> Vec<RecordedCall> {
        self.state.lock().unwrap().calls.clone()
    }

    fn record(&self, method: &str, args: Vec<String>) {
        self.state.lock().unwrap().calls.push(RecordedCall {
            method: method.to_string(),
            args,
        });
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    async fn check_available(&self) -> Result<()> {
        self.record("check_available", vec![]);
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record("run", vec![spec.image.clone()]);
        Ok(ContainerHandle { id: "mock-id".to_string(), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record("create", vec![spec.image.clone()]);
        Ok(ContainerHandle { id: "mock-id".to_string(), name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> {
        self.record("start", vec![id.to_string()]);
        Ok(())
    }
    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<()> {
        self.record("stop", vec![id.to_string()]);
        Ok(())
    }
    async fn remove(&self, id: &str, _force: bool) -> Result<()> {
        self.record("remove", vec![id.to_string()]);
        Ok(())
    }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> {
        self.record("list", vec![]);
        Ok(vec![])
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.record("inspect", vec![id.to_string()]);
        Ok(ContainerInfo {
            id: id.to_string(),
            name: id.to_string(),
            image: "mock-image".to_string(),
            status: "running".to_string(),
            ports: vec![],
            created: "".to_string(),
        })
    }
    async fn logs(&self, id: &str, _tail: Option<u32>) -> Result<ContainerLogs> {
        self.record("logs", vec![id.to_string()]);
        Ok(ContainerLogs { stdout: "".to_string(), stderr: "".to_string() })
    }
    async fn exec(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.record("exec", vec![id.to_string(), cmd.join(" ")]);
        Ok(ContainerLogs { stdout: "".to_string(), stderr: "".to_string() })
    }
    async fn build(&self, _spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.record("build", vec![image_name.to_string()]);
        Ok(())
    }
    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.record("pull_image", vec![reference.to_string()]);
        Ok(())
    }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.record("list_images", vec![]);
        Ok(vec![])
    }
    async fn remove_image(&self, reference: &str, _force: bool) -> Result<()> {
        self.record("remove_image", vec![reference.to_string()]);
        Ok(())
    }
    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> {
        self.record("create_network", vec![name.to_string()]);
        Ok(())
    }
    async fn remove_network(&self, name: &str) -> Result<()> {
        self.record("remove_network", vec![name.to_string()]);
        Ok(())
    }
    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> {
        self.record("create_volume", vec![name.to_string()]);
        Ok(())
    }
    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.record("remove_volume", vec![name.to_string()]);
        Ok(())
    }
}
