use crate::error::{ComposeError, Result};
use crate::types::{
    ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use std::time::Duration;
use std::sync::OnceLock;

pub use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

/// Minimal network creation config — driver and labels only.
/// The compose layer converts ComposeNetwork → NetworkConfig before calling the backend.
#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

/// Minimal volume creation config — driver and labels only.
#[derive(Debug, Clone, Default)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct SecurityProfile {
    pub seccomp: Option<String>,
    pub readonly_rootfs: bool,
}

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    async fn check_available(&self) -> Result<()>;
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle>;
    async fn wait(&self, id: &str) -> Result<i32>;
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
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, true)
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, false)
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
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels {
            args.extend(["--label".into(), format!("{k}={v}")]);
        }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
        args.push(name.into());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "rm".into(), name.into()]
    }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels {
            args.extend(["--label".into(), format!("{k}={v}")]);
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

pub fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args = if include_detach {
        vec!["run".into(), "--detach".into()]
    } else {
        vec!["create".into()]
    };
    if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
    for port in spec.ports.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-p".into(), port.clone()]); }
    for vol in spec.volumes.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-v".into(), vol.clone()]); }
    for (k, v) in spec.env.as_ref().iter().flat_map(|m| m.iter()) { args.extend(["-e".into(), format!("{k}={v}")]); }
    if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
    if spec.rm.unwrap_or(false) && include_detach { args.push("--rm".into()); }
    if let Some(ep) = &spec.entrypoint {
        args.push("--entrypoint".into());
        args.push(ep.join(" "));
    }
    args.push(spec.image.clone());
    for c in spec.cmd.as_ref().iter().flat_map(|v| v.iter()) { args.push(c.clone()); }
    args
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
    fn protocol_name(&self) -> &str { "docker-compatible" }
}

pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, false) // apple/container doesn't support --detach
    }
}

pub struct LimaProtocol {
    pub instance: String,
}

impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }

    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
}

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, args: &[String]) -> Result<(String, String)> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(args);

        let output = cmd.output()
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
impl<P: CliProtocol + Send + Sync> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.protocol.protocol_name()
    }

    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.arg("--version")
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

    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle> {
        let mut args = self.protocol.run_args(spec);
        // Inject security flags before the image name
        let image_idx = args.iter().position(|a| a == &spec.image).unwrap_or(args.len() - 1);

        let mut security_args = Vec::new();
        if profile.readonly_rootfs {
            security_args.push("--read-only".to_string());
        }
        if let Some(seccomp) = &profile.seccomp {
            security_args.extend(["--security-opt".into(), format!("seccomp={}", seccomp)]);
        }

        for (i, arg) in security_args.into_iter().enumerate() {
            args.insert(image_idx + i, arg);
        }

        let (stdout, _) = self.exec_raw(&args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn wait(&self, id: &str) -> Result<i32> {
        let args = vec!["wait".to_string(), id.to_string()];
        let (stdout, _) = self.exec_raw(&args).await?;
        stdout.trim().parse::<i32>().map_err(|_| ComposeError::BackendError {
            code: -1,
            message: "Failed to parse exit code from wait".into()
        })
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

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_network_args(name);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        self.exec_raw(&args).await.map(|_| ())
    }
}

static DETECTED_BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

pub async fn detect_backend() -> std::result::Result<Arc<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    if let Some(backend) = DETECTED_BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let backend = do_detect_backend().await?;
    let arc_backend: Arc<dyn ContainerBackend> = Arc::from(backend);
    let _ = DETECTED_BACKEND.set(Arc::clone(&arc_backend));

    Ok(arc_backend)
}

#[async_trait]
impl ContainerBackend for Box<dyn ContainerBackend> {
    fn backend_name(&self) -> &str { self.as_ref().backend_name() }
    async fn check_available(&self) -> Result<()> { self.as_ref().check_available().await }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> { self.as_ref().run(spec).await }
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle> { self.as_ref().run_with_security(spec, profile).await }
    async fn wait(&self, id: &str) -> Result<i32> { self.as_ref().wait(id).await }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> { self.as_ref().create(spec).await }
    async fn start(&self, id: &str) -> Result<()> { self.as_ref().start(id).await }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.as_ref().stop(id, timeout).await }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.as_ref().remove(id, force).await }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> { self.as_ref().list(all).await }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> { self.as_ref().inspect(id).await }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> { self.as_ref().logs(id, tail).await }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> { self.as_ref().exec(id, cmd, env, workdir).await }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.as_ref().pull_image(reference).await }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { self.as_ref().list_images().await }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.as_ref().remove_image(reference, force).await }
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> { self.as_ref().create_network(name, config).await }
    async fn remove_network(&self, name: &str) -> Result<()> { self.as_ref().remove_network(name).await }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> { self.as_ref().create_volume(name, config).await }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.as_ref().remove_volume(name).await }
}

#[async_trait]
impl ContainerBackend for Arc<dyn ContainerBackend> {
    fn backend_name(&self) -> &str { self.as_ref().backend_name() }
    async fn check_available(&self) -> Result<()> { self.as_ref().check_available().await }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> { self.as_ref().run(spec).await }
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle> { self.as_ref().run_with_security(spec, profile).await }
    async fn wait(&self, id: &str) -> Result<i32> { self.as_ref().wait(id).await }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> { self.as_ref().create(spec).await }
    async fn start(&self, id: &str) -> Result<()> { self.as_ref().start(id).await }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.as_ref().stop(id, timeout).await }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.as_ref().remove(id, force).await }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> { self.as_ref().list(all).await }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> { self.as_ref().inspect(id).await }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> { self.as_ref().logs(id, tail).await }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> { self.as_ref().exec(id, cmd, env, workdir).await }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.as_ref().pull_image(reference).await }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { self.as_ref().list_images().await }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.as_ref().remove_image(reference, force).await }
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> { self.as_ref().create_network(name, config).await }
    async fn remove_network(&self, name: &str) -> Result<()> { self.as_ref().remove_network(name).await }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> { self.as_ref().create_volume(name, config).await }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.as_ref().remove_volume(name).await }
}

async fn do_detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
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

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    let which_bin = |name: &str| -> std::result::Result<PathBuf, String> {
        which::which(name).map_err(|_| format!("{} not found", name))
    };

    match name {
        "apple/container" => {
            let bin = which_bin("container")?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
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
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "orbstack" => {
            let bin = which_bin("orb").or_else(|_| which_bin("docker")).map_err(|_| "orbstack not found")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "colima" => {
            let bin = which_bin("colima")?;
            let out = Command::new(&bin).arg("status").output().await.map_err(|_| "colima status failed")?;
            if !String::from_utf8_lossy(&out.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let dbin = which_bin("docker").map_err(|_| "docker cli not found for colima")?;
            Ok(Box::new(CliBackend::new(dbin, DockerProtocol)))
        }
        "lima" => {
            let bin = which_bin("limactl")?;
            let out = Command::new(&bin).args(&["list", "--json"]).output().await.map_err(|_| "limactl list failed")?;
            let instance = String::from_utf8_lossy(&out.stdout).lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["status"] == "Running")
                .and_then(|v| v["name"].as_str().map(|s| s.to_string()))
                .ok_or("no running lima instance")?;
            Ok(Box::new(CliBackend::new(bin, LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which_bin("nerdctl")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which_bin("docker")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        _ => Err("unknown backend".into()),
    }
}
