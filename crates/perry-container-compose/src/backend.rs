use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    async fn check_available(&self) -> Result<()>;
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn start(&self, id: &str) -> Result<()>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;
    async fn remove(&self, id: &str, force: bool) -> Result<()>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

pub trait CliProtocol: Send + Sync {
    fn subcommand_prefix(&self) -> Option<&str> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String>;
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn inspect_args(&self, id: &str) -> Vec<String>;
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String>;
    fn pull_image_args(&self, reference: &str) -> Vec<String>;
    fn list_images_args(&self) -> Vec<String>;
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String>;
    fn remove_network_args(&self, name: &str) -> Vec<String>;
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String>;
    fn remove_volume_args(&self, name: &str) -> Vec<String>;

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>>;
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>>;
    fn parse_container_id(&self, stdout: &str) -> Result<String>;
}

#[derive(Debug, Deserialize)]
struct DockerListEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "Status", alias = "State", default)]
    status: String,
    #[serde(rename = "Ports", default)]
    ports: Vec<String>,
    #[serde(rename = "Created", alias = "CreatedAt", default)]
    created: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectOutput {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Config")]
    config: DockerInspectConfig,
    #[serde(rename = "State")]
    state: DockerInspectState,
    #[serde(rename = "Created")]
    created: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectConfig {
    #[serde(rename = "Image")]
    image: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectState {
    #[serde(rename = "Status")]
    status: String,
}

#[derive(Debug, Deserialize)]
struct DockerImageEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Repositories", alias = "Repository", default)]
    repository: String,
    #[serde(rename = "Tag", default)]
    tag: String,
    #[serde(rename = "Size", default)]
    size: u64,
    #[serde(rename = "Created", alias = "CreatedAt", default)]
    created: String,
}

pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into(), "--detach".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        for port in spec.ports.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.as_ref().iter().flat_map(|m| m.iter()) { args.extend(["-e".into(), format!("{k}={v}")]); }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if let Some(ep) = &spec.entrypoint {
            args.push("--entrypoint".into());
            args.push(ep.join(" "));
        }
        args.push(spec.image.clone());
        for c in spec.cmd.as_ref().iter().flat_map(|v| v.iter()) { args.push(c.clone()); }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        for port in spec.ports.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.as_ref().iter().flat_map(|m| m.iter()) { args.extend(["-e".into(), format!("{k}={v}")]); }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if let Some(ep) = &spec.entrypoint {
            args.push("--entrypoint".into());
            args.push(ep.join(" "));
        }
        args.push(spec.image.clone());
        for c in spec.cmd.as_ref().iter().flat_map(|v| v.iter()) { args.push(c.clone()); }
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> {
        vec!["start".into(), id.into()]
    }

    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        args
    }

    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        args
    }

    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("--all".into()); }
        args
    }

    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail { args.extend(["--tail".into(), t.to_string()]); }
        args.push(id.into());
        args
    }

    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(w) = workdir { args.extend(["--workdir".into(), w.into()]); }
        if let Some(e) = env {
            for (k, v) in e { args.extend(["-e".into(), format!("{k}={v}")]); }
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }

    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        vec!["pull".into(), reference.into()]
    }

    fn list_images_args(&self) -> Vec<String> {
        vec!["images".into(), "--format".into(), "json".into()]
    }

    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }

    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.extend(["--label".into(), format!("{k}={v}")]);
            }
        }
        args.push(name.into());
        args
    }

    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "rm".into(), name.into()]
    }

    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.extend(["--label".into(), format!("{k}={v}")]);
            }
        }
        args.push(name.into());
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        let entries: Vec<DockerListEntry> = stdout.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(entries.into_iter().map(|e| ContainerInfo {
            id: e.id,
            name: e.names.first().cloned().unwrap_or_default(),
            image: e.image,
            status: e.status,
            ports: e.ports,
            created: e.created,
        }).collect())
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        let entries: Vec<DockerInspectOutput> = serde_json::from_str(stdout)?;
        let e = entries.into_iter().next().ok_or_else(|| ComposeError::NotFound("Inspect output empty".into()))?;
        Ok(ContainerInfo {
            id: e.id,
            name: e.name,
            image: e.config.image,
            status: e.state.status,
            ports: vec![],
            created: e.created,
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        let entries: Vec<DockerImageEntry> = stdout.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(entries.into_iter().map(|e| ImageInfo {
            id: e.id,
            repository: e.repository,
            tag: e.tag,
            size: e.size,
            created: e.created,
        }).collect())
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }
}

pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
        for port in spec.ports.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.as_ref().iter().flat_map(|m| m.iter()) { args.extend(["-e".into(), format!("{k}={v}")]); }
        args.push(spec.image.clone());
        for c in spec.cmd.as_ref().iter().flat_map(|v| v.iter()) { args.push(c.clone()); }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { DockerProtocol.create_args(spec) }
    fn start_args(&self, id: &str) -> Vec<String> { DockerProtocol.start_args(id) }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> { DockerProtocol.stop_args(id, timeout) }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> { DockerProtocol.remove_args(id, force) }
    fn list_args(&self, all: bool) -> Vec<String> { DockerProtocol.list_args(all) }
    fn inspect_args(&self, id: &str) -> Vec<String> { DockerProtocol.inspect_args(id) }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> { DockerProtocol.logs_args(id, tail) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> { DockerProtocol.exec_args(id, cmd, env, workdir) }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { DockerProtocol.pull_image_args(reference) }
    fn list_images_args(&self) -> Vec<String> { DockerProtocol.list_images_args() }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> { DockerProtocol.remove_image_args(reference, force) }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> { DockerProtocol.create_network_args(name, config) }
    fn remove_network_args(&self, name: &str) -> Vec<String> { DockerProtocol.remove_network_args(name) }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> { DockerProtocol.create_volume_args(name, config) }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { DockerProtocol.remove_volume_args(name) }
    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> { DockerProtocol.parse_list_images_output(stdout) }
    fn parse_container_id(&self, stdout: &str) -> Result<String> { DockerProtocol.parse_container_id(stdout) }
}

pub struct LimaProtocol {
    pub instance: String,
}

impl CliProtocol for LimaProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.run_args(spec));
        args
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_args(spec));
        args
    }
    fn start_args(&self, id: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.start_args(id));
        args
    }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.stop_args(id, timeout));
        args
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_args(id, force));
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.list_args(all));
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.inspect_args(id));
        args
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.logs_args(id, tail));
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.exec_args(id, cmd, env, workdir));
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.pull_image_args(reference));
        args
    }
    fn list_images_args(&self) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.list_images_args());
        args
    }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_image_args(reference, force));
        args
    }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_network_args(name, config));
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_network_args(name));
        args
    }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_volume_args(name, config));
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_volume_args(name));
        args
    }
    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> { DockerProtocol.parse_list_images_output(stdout) }
    fn parse_container_id(&self, stdout: &str) -> Result<String> { DockerProtocol.parse_container_id(stdout) }
}

pub struct CliBackend {
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

impl CliBackend {
    pub fn new(bin: PathBuf, protocol: Box<dyn CliProtocol>) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, args: &[String]) -> Result<(String, String)> {
        let output = Command::new(&self.bin)
            .args(args)
            .output()
            .await
            .map_err(ComposeError::IoError)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok((stdout, stderr))
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr,
            })
        }
    }
}

#[async_trait]
impl ContainerBackend for CliBackend {
    fn backend_name(&self) -> &str {
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        Command::new(&self.bin)
            .arg("--version")
            .output()
            .await
            .map_err(ComposeError::IoError)
            .map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let (stdout, _) = self.exec_raw(&args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let (stdout, _) = self.exec_raw(&args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = self.protocol.start_args(id);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = self.protocol.stop_args(id, timeout);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_args(id, force);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = self.protocol.list_args(all);
        let (stdout, _) = self.exec_raw(&args).await?;
        self.protocol.parse_list_output(&stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = self.protocol.inspect_args(id);
        let (stdout, _) = self.exec_raw(&args).await?;
        self.protocol.parse_inspect_output(&stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = self.protocol.logs_args(id, tail);
        let (stdout, stderr) = self.exec_raw(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let args = self.protocol.exec_args(id, cmd, env, workdir);
        let (stdout, stderr) = self.exec_raw(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = self.protocol.pull_image_args(reference);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = self.protocol.list_images_args();
        let (stdout, _) = self.exec_raw(&args).await?;
        self.protocol.parse_list_images_output(&stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_image_args(reference, force);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_network_args(name);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        self.exec_raw(&args).await.map(|_| ())
    }
}

pub async fn detect_backend() -> std::result::Result<CliBackend, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map_err(|reason| vec![BackendProbeResult { name: name.clone(), available: false, reason }]);
    }

    let candidates = platform_candidates();
    let mut results = Vec::new();

    for candidate in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: "probe timed out".into() }),
        }
    }

    Err(results)
}

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"]
    } else {
        &["podman", "nerdctl", "docker"]
    }
}

async fn probe_candidate(name: &str) -> std::result::Result<CliBackend, String> {
    let which_bin = |name: &str| -> std::result::Result<PathBuf, String> {
        which::which(name).map_err(|_| format!("{} not found", name))
    };

    match name {
        "apple/container" => {
            let bin = which_bin("container")?;
            Ok(CliBackend::new(bin, Box::new(AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which_bin("podman")?;
            if cfg!(target_os = "macos") {
                let out = Command::new(&bin).args(&["machine", "list", "--format", "json"]).output().await.map_err(|_| "podman machine list failed")?;
                let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| "invalid podman output")?;
                if !json.as_array().map(|a| a.iter().any(|m| m["Running"].as_bool().unwrap_or(false))).unwrap_or(false) {
                    return Err("no podman machine running".into());
                }
            }
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "orbstack" => {
            let bin = which_bin("orb").or_else(|_| which_bin("docker")).map_err(|_| "orbstack not found")?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "colima" => {
            let bin = which_bin("colima")?;
            let out = Command::new(&bin).arg("status").output().await.map_err(|_| "colima status failed")?;
            if !String::from_utf8_lossy(&out.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let dbin = which_bin("docker").map_err(|_| "docker cli not found for colima")?;
            Ok(CliBackend::new(dbin, Box::new(DockerProtocol)))
        }
        "lima" => {
            let bin = which_bin("limactl")?;
            let out = Command::new(&bin).args(&["list", "--json"]).output().await.map_err(|_| "limactl list failed")?;
            let instance = String::from_utf8_lossy(&out.stdout).lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["status"] == "Running")
                .and_then(|v| v["name"].as_str().map(|s| s.to_string()))
                .ok_or("no running lima instance")?;
            Ok(CliBackend::new(bin, Box::new(LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which_bin("nerdctl")?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "docker" => {
            let bin = which_bin("docker")?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        _ => Err("unknown backend".into()),
    }
}
