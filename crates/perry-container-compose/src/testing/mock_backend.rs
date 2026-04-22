use crate::backend::ContainerBackend;
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct MockBackend {
    pub calls: Mutex<Vec<String>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self { calls: Mutex::new(Vec::new()) }
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push(format!("run:{}", spec.image));
        Ok(ContainerHandle { id: "mock-id".into(), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push(format!("create:{}", spec.image));
        Ok(ContainerHandle { id: "mock-id".into(), name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> {
        self.calls.lock().unwrap().push(format!("start:{}", id));
        Ok(())
    }
    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<()> {
        self.calls.lock().unwrap().push(format!("stop:{}", id));
        Ok(())
    }
    async fn remove(&self, id: &str, _force: bool) -> Result<()> {
        self.calls.lock().unwrap().push(format!("remove:{}", id));
        Ok(())
    }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> { Ok(vec![]) }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        Ok(ContainerInfo {
            id: id.into(),
            name: id.into(),
            image: "mock-image".into(),
            status: "running".into(),
            ports: vec![],
            created: "".into(),
        })
    }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }
    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> {
        self.calls.lock().unwrap().push(format!("create_network:{}", name));
        Ok(())
    }
    async fn remove_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> {
        self.calls.lock().unwrap().push(format!("create_volume:{}", name));
        Ok(())
    }
    async fn remove_volume(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn inspect_network(&self, _name: &str) -> Result<()> { Ok(()) }
}
