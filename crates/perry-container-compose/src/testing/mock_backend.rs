use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::error::Result;
use crate::types::{ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum RecordedCall {
    Run(ContainerSpec),
    Create(ContainerSpec),
    Start(String),
    Stop(String, Option<u32>),
    Remove(String, bool),
    List(bool),
    Inspect(String),
    Logs(String, Option<u32>),
    Exec(String, Vec<String>),
    Build(String),
}

pub struct MockBackend {
    pub name: String,
    pub calls: Arc<Mutex<Vec<RecordedCall>>>,
    pub responses: Arc<Mutex<VecDeque<Result<serde_json::Value>>>>,
}

impl MockBackend {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn push_ok<T: serde::Serialize>(&self, val: T) {
        self.responses.lock().unwrap().push_back(Ok(serde_json::to_value(val).unwrap()));
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { &self.name }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn build(&self, _spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Build(image_name.to_string()));
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push(RecordedCall::Run(spec.clone()));
        Ok(ContainerHandle { id: "mock-id".to_string(), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push(RecordedCall::Create(spec.clone()));
        Ok(ContainerHandle { id: "mock-id".to_string(), name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Start(id.to_string()));
        Ok(())
    }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Stop(id.to_string(), timeout));
        Ok(())
    }
    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Remove(id.to_string(), force));
        Ok(())
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.calls.lock().unwrap().push(RecordedCall::List(all));
        Ok(Vec::new())
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.calls.lock().unwrap().push(RecordedCall::Inspect(id.to_string()));
        Ok(ContainerInfo { id: id.to_string(), name: id.to_string(), image: "img".to_string(), status: "running".to_string(), ports: Vec::new(), created: "".to_string() })
    }
    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        Ok(ImageInfo { id: "id".to_string(), repository: reference.to_string(), tag: "latest".to_string(), size: 0, created: "".to_string() })
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push(RecordedCall::Logs(id.to_string(), tail));
        Ok(ContainerLogs { stdout: "".to_string(), stderr: "".to_string() })
    }
    async fn wait(&self, _id: &str) -> Result<i32> { Ok(0) }
    async fn exec(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push(RecordedCall::Exec(id.to_string(), cmd.to_vec()));
        Ok(ContainerLogs { stdout: "".to_string(), stderr: "".to_string() })
    }
    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(Vec::new()) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, _name: &str, _config: &NetworkConfig) -> Result<()> { Ok(()) }
    async fn remove_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, _name: &str, _config: &VolumeConfig) -> Result<()> { Ok(()) }
    async fn remove_volume(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn inspect_network(&self, _name: &str) -> Result<()> { Ok(()) }
}
