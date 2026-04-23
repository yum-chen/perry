use async_trait::async_trait;
use perry_container_compose::backend::{ContainerBackend, NetworkConfig, VolumeConfig, SecurityProfile};
use perry_container_compose::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo,
    ContainerSpec, ComposeServiceBuild
};
use perry_container_compose::error::{ComposeError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct MockBackendState {
    pub containers: Vec<String>,
    pub networks: Vec<String>,
    pub volumes: Vec<String>,
    pub actions: Vec<String>,
    pub fail_on_run: Option<String>, // Service name to fail on
}

#[derive(Clone, Default)]
pub struct MockBackend {
    pub state: Arc<Mutex<MockBackendState>>,
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }

    async fn check_available(&self) -> Result<()> { Ok(()) }

    async fn build(&self, _spec: &ComposeServiceBuild, _image_name: &str) -> Result<()> {
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut state = self.state.lock().unwrap();
        let name = spec.name.clone().unwrap_or_else(|| "unnamed".to_string());

        if let Some(fail_name) = &state.fail_on_run {
            if name.contains(fail_name) {
                return Err(ComposeError::ServiceStartupFailed {
                    service: name,
                    message: "Mock failure".to_string(),
                });
            }
        }

        state.actions.push(format!("run:{}", name));
        state.containers.push(name.clone());
        Ok(ContainerHandle { id: name.clone(), name: Some(name) })
    }

    async fn run_with_security(&self, spec: &ContainerSpec, _profile: &SecurityProfile) -> Result<ContainerHandle> {
        self.run(spec).await
    }

    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "id".into(), name: None }) }
    async fn start(&self, _id: &str) -> Result<()> { Ok(()) }
    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.actions.push(format!("stop:{}", id));
        Ok(())
    }
    async fn remove(&self, id: &str, _force: bool) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.actions.push(format!("remove:{}", id));
        state.containers.retain(|c| c != id);
        Ok(())
    }

    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> {
        let state = self.state.lock().unwrap();
        let mut infos = Vec::new();
        for id in &state.containers {
            infos.push(ContainerInfo {
                id: id.clone(),
                name: id.clone(),
                image: "mock-image".to_string(),
                status: "running".to_string(),
                ports: vec![],
                created: "2025-01-01T00:00:00Z".to_string(),
            })
        }
        Ok(infos)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let state = self.state.lock().unwrap();
        if state.containers.contains(&id.to_string()) {
            Ok(ContainerInfo {
                id: id.to_string(),
                name: id.to_string(),
                image: "mock-image".to_string(),
                status: "running".to_string(),
                ports: vec![],
                created: "2025-01-01T00:00:00Z".to_string(),
            })
        } else {
            Err(ComposeError::NotFound(id.to_string()))
        }
    }

    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "logs".into(), stderr: "".into() })
    }

    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "exec".into(), stderr: "".into() })
    }

    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }

    async fn create_network(&self, name: &str, _config: &NetworkConfig) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.actions.push(format!("create_network:{}", name));
        state.networks.push(name.to_string());
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.actions.push(format!("remove_network:{}", name));
        state.networks.retain(|n| n != name);
        Ok(())
    }

    async fn create_volume(&self, name: &str, _config: &VolumeConfig) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.actions.push(format!("create_volume:{}", name));
        state.volumes.push(name.to_string());
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.actions.push(format!("remove_volume:{}", name));
        state.volumes.retain(|v| v != name);
        Ok(())
    }

    async fn inspect_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn inspect_volume(&self, _name: &str) -> Result<()> { Ok(()) }
}
