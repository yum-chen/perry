use crate::error::Result;
use crate::backend::{ContainerBackend, ExecutionStrategy, NetworkConfig, VolumeConfig};
use crate::types::{ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, IsolationLevel};
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
    PullImage(String),
    ListImages,
    RemoveImage(String, bool),
    CreateNetwork(String),
    RemoveNetwork(String),
    CreateVolume(String),
    RemoveVolume(String),
    InspectNetwork(String),
}

pub struct MockBackend {
    pub calls: Arc<Mutex<Vec<RecordedCall>>>,
    pub responses: Arc<Mutex<VecDeque<Result<serde_json::Value>>>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn push_response<T: serde::Serialize>(&self, resp: Result<T>) {
        let val = resp.map(|t| serde_json::to_value(t).unwrap());
        self.responses.lock().unwrap().push_back(val);
    }

    fn record(&self, call: RecordedCall) {
        self.calls.lock().unwrap().push(call);
    }

    fn pop_response<T: serde::de::DeserializeOwned + Default>(&self) -> Result<T> {
        let mut resps = self.responses.lock().unwrap();
        match resps.pop_front() {
            Some(Ok(v)) => Ok(serde_json::from_value(v).unwrap()),
            Some(Err(e)) => Err(e),
            None => Ok(T::default()),
        }
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    fn strategy(&self) -> ExecutionStrategy { ExecutionStrategy::CliExec { bin: "mock".into() } }
    fn isolation_level(&self) -> IsolationLevel { IsolationLevel::Container }

    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record(RecordedCall::Run(spec.clone()));
        self.pop_response()
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record(RecordedCall::Create(spec.clone()));
        self.pop_response()
    }
    async fn start(&self, id: &str) -> Result<()> {
        self.record(RecordedCall::Start(id.to_string()));
        self.pop_response()
    }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.record(RecordedCall::Stop(id.to_string(), timeout));
        self.pop_response()
    }
    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::Remove(id.to_string(), force));
        self.pop_response()
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.record(RecordedCall::List(all));
        self.pop_response()
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.record(RecordedCall::Inspect(id.to_string()));
        self.pop_response()
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.record(RecordedCall::Logs(id.to_string(), tail));
        self.pop_response()
    }
    async fn exec(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.record(RecordedCall::Exec(id.to_string(), cmd.to_vec()));
        self.pop_response()
    }
    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.record(RecordedCall::PullImage(reference.to_string()));
        self.pop_response()
    }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.record(RecordedCall::ListImages);
        self.pop_response()
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::RemoveImage(reference.to_string(), force));
        self.pop_response()
    }
    async fn create_network(&self, name: &str, _config: &NetworkConfig) -> Result<()> {
        self.record(RecordedCall::CreateNetwork(name.to_string()));
        self.pop_response()
    }
    async fn remove_network(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveNetwork(name.to_string()));
        self.pop_response()
    }
    async fn create_volume(&self, name: &str, _config: &VolumeConfig) -> Result<()> {
        self.record(RecordedCall::CreateVolume(name.to_string()));
        self.pop_response()
    }
    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveVolume(name.to_string()));
        self.pop_response()
    }
    async fn build(&self, _spec: &crate::types::ComposeServiceBuild, _image_name: &str) -> Result<()> {
        Ok(())
    }
    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::InspectNetwork(name.to_string()));
        self.pop_response()
    }
}
