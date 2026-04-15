//! Container backend abstraction and implementation.
//!
//! Separates the `ContainerBackend` async trait from the `CliProtocol` trait,
//! allowing different container runtimes (podman, docker, apple-container, etc.)
//! to be supported by the same generic `CliBackend` executor.

use crate::error::{BackendProbeResult, ComposeError, Result};
use crate::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// Layer 1: The public contract — what operations exist, completely runtime-agnostic.
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Backend name for display (e.g. "apple/container", "podman", "docker")
    fn backend_name(&self) -> &str;

    /// Check whether the backend binary is available and functional.
    async fn check_available(&self) -> Result<()>;

    /// Run a container (create + start). Returns a handle.
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Create a container (without starting it).
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Start an existing stopped container.
    async fn start(&self, id: &str) -> Result<()>;

    /// Stop a running container.
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;

    /// Remove a container.
    async fn remove(&self, id: &str, force: bool) -> Result<()>;

    /// List all containers.
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;

    /// Inspect a container.
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;

    /// Fetch logs from a container.
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;

    /// Execute a command inside a running container.
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs>;

    /// Build an image from a context.
    async fn build(
        &self,
        spec: &crate::types::ComposeServiceBuild,
        image_name: &str,
    ) -> Result<()>;

    /// Pull an image.
    async fn pull_image(&self, reference: &str) -> Result<()>;

    /// List images.
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;

    /// Remove an image.
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;

    /// Create a network.
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;

    /// Remove a network.
    async fn remove_network(&self, name: &str) -> Result<()>;

    /// Create a volume.
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;

    /// Remove a volume.
    async fn remove_volume(&self, name: &str) -> Result<()>;

    /// Inspect a network.
    async fn inspect_network(&self, name: &str) -> Result<()>;

    async fn wait(&self, id: &str) -> Result<i32>;
    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo>;
}

/// Layer 2: CLI Protocol trait.
/// Separates *command building* from *command execution*.
pub trait CliProtocol: Send + Sync {
    /// Identifies this protocol family (used in logs and error messages).
    fn protocol_name(&self) -> &str;

    /// Optional prefix prepended before every subcommand.
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        None
    }

    // ── Argument builders — all have Docker-compatible defaults ───────────

    fn build_args(
        &self,
        spec: &crate::types::ComposeServiceBuild,
        image_name: &str,
    ) -> Vec<String> {
        let mut cmd_args = vec!["build".into(), "-t".into(), image_name.into()];
        if let Some(df) = &spec.containerfile {
            cmd_args.extend(["-f".into(), df.into()]);
        }
        if let Some(ba) = &spec.args {
            for (k, v) in ba.to_map() {
                cmd_args.extend(["--build-arg".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(t) = &spec.target {
            cmd_args.extend(["--target".into(), t.into()]);
        }
        if let Some(n) = &spec.network {
            cmd_args.extend(["--network".into(), n.into()]);
        }
        cmd_args.push(spec.context.as_deref().unwrap_or(".").into());
        cmd_args
    }

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
        if let Some(t) = timeout {
            args.extend(["--time".into(), t.to_string()]);
        }
        args.push(id.into());
        args
    }

    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force {
            args.push("-f".into());
        }
        args.push(id.into());
        args
    }

    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all {
            args.push("--all".into());
        }
        args
    }

    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail {
            args.extend(["--tail".into(), t.to_string()]);
        }
        args.push(id.into());
        args
    }

    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(envs) = env {
            for (k, v) in envs {
                args.extend(["-e".into(), format!("{k}={v}")]);
            }
        }
        if let Some(wd) = workdir {
            args.extend(["--workdir".into(), wd.into()]);
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
        if force {
            args.push("-f".into());
        }
        args.push(reference.into());
        args
    }

    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(driver) = &config.driver {
            args.extend(["--driver".into(), driver.clone()]);
        }
        for (k, v) in &config.labels {
            args.extend(["--label".into(), format!("{}={}", k, v)]);
        }
        if config.internal {
            args.push("--internal".into());
        }
        args.push(name.into());
        args
    }

    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "rm".into(), name.into()]
    }

    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(driver) = &config.driver {
            args.extend(["--driver".into(), driver.clone()]);
        }
        for (k, v) in &config.labels {
            args.extend(["--label".into(), format!("{}={}", k, v)]);
        }
        args.push(name.into());
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    fn inspect_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "inspect".into(), name.into()]
    }

    fn wait_args(&self, id: &str) -> Vec<String> {
        vec!["wait".into(), id.into()]
    }

    fn inspect_image_args(&self, reference: &str) -> Vec<String> {
        vec![
            "image".into(),
            "inspect".into(),
            "--format".into(),
            "json".into(),
            reference.into(),
        ]
    }

    // ── Output parsers — all have Docker JSON defaults ────────────────────

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        let entries: Vec<serde_json::Value> = stdout
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        Ok(entries
            .into_iter()
            .map(|e| ContainerInfo {
                id: e["ID"].as_str().unwrap_or_default().to_string(),
                name: e["Names"]
                    .as_str()
                    .or_else(|| e["Names"].as_array().and_then(|a| a[0].as_str()))
                    .unwrap_or_default()
                    .to_string(),
                image: e["Image"].as_str().unwrap_or_default().to_string(),
                status: e["Status"].as_str().unwrap_or_default().to_string(),
                ports: vec![e["Ports"].as_str().unwrap_or_default().to_string()],
                labels: e["Labels"]
                    .as_object()
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                            .collect()
                    })
                    .or_else(|| {
                        e["Labels"].as_str().map(|s| {
                            s.split(',')
                                .filter_map(|pair| pair.split_once('='))
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect()
                        })
                    })
                    .unwrap_or_default(),
                created: e["CreatedAt"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        let val: serde_json::Value = serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
        let e = if val.is_array() { &val[0] } else { &val };

        let labels = if let Some(obj) = e["Config"]["Labels"].as_object() {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                .collect()
        } else {
            HashMap::new()
        };

        Ok(ContainerInfo {
            id: e["Id"].as_str().unwrap_or_default().to_string(),
            name: e["Name"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_string(),
            image: e["Config"]["Image"].as_str().unwrap_or_default().to_string(),
            status: e["State"]["Status"].as_str().unwrap_or_default().to_string(),
            ports: vec![],
            labels,
            created: e["Created"].as_str().unwrap_or_default().to_string(),
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        let entries: Vec<serde_json::Value> = stdout
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        Ok(entries
            .into_iter()
            .map(|e| ImageInfo {
                id: e["ID"].as_str().unwrap_or_default().to_string(),
                repository: e["Repository"].as_str().unwrap_or_default().to_string(),
                tag: e["Tag"].as_str().unwrap_or_default().to_string(),
                size: 0,
                created: e["CreatedAt"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }

    fn parse_inspect_image_output(&self, stdout: &str) -> Result<ImageInfo> {
        let val: serde_json::Value = serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
        let e = if val.is_array() { &val[0] } else { &val };

        Ok(ImageInfo {
            id: e["Id"].as_str().unwrap_or_default().to_string(),
            repository: String::new(),
            tag: String::new(),
            size: e["Size"].as_u64().unwrap_or(0),
            created: e["Created"].as_str().unwrap_or_default().to_string(),
        })
    }
}

pub fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args = vec!["run".to_string()];
    if include_detach {
        args.push("--detach".into());
    }
    if let Some(name) = &spec.name {
        args.extend(["--name".into(), name.clone()]);
    }
    if let Some(ports) = &spec.ports {
        for port in ports {
            args.extend(["-p".into(), port.clone()]);
        }
    }
    if let Some(volumes) = &spec.volumes {
        for vol in volumes {
            args.extend(["-v".into(), vol.clone()]);
        }
    }
    if let Some(env) = &spec.env {
        for (k, v) in env {
            args.extend(["-e".into(), format!("{k}={v}")]);
        }
    }
    if let Some(labels) = &spec.labels {
        for (k, v) in labels {
            args.extend(["--label".into(), format!("{k}={v}")]);
        }
    }
    if let Some(net) = &spec.network {
        args.extend(["--network".into(), net.clone()]);
    }
    if spec.rm.unwrap_or(false) {
        args.push("--rm".into());
    }
    if spec.read_only.unwrap_or(false) {
        args.push("--read-only".into());
    }
    if let Some(ep) = &spec.entrypoint {
        args.extend(["--entrypoint".into(), ep.join(" ")]);
    }
    args.push(spec.image.clone());
    if let Some(cmd) = &spec.cmd {
        args.extend(cmd.iter().cloned());
    }
    args
}

/// Docker-compatible CLI protocol implementation.
pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str {
        "docker-compatible"
    }
}

/// Apple Container CLI protocol implementation.
pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str {
        "apple/container"
    }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, false)
    }
}

/// Lima CLI protocol implementation.
pub struct LimaProtocol {
    pub instance: String,
}

impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str {
        "lima"
    }

    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

/// Generic CLI backend implementation.
pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
}

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

pub trait SecurityProfile: Send + Sync {}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, subcommand_args: Vec<String>) -> Result<CliOutput> {
        let mut cmd = tokio::process::Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(subcommand_args);

        let output = cmd.output().await.map_err(ComposeError::IoError)?;

        if output.status.success() {
            Ok(CliOutput {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let out = self.exec_raw(args).await?;
        Ok(out.stdout)
    }
}

struct CliOutput {
    stdout: String,
    stderr: String,
}

#[async_trait]
impl<P: CliProtocol + Send + Sync> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.protocol.protocol_name()
    }

    async fn check_available(&self) -> Result<()> {
        let args = vec!["--version".to_string()];
        self.exec_ok(args).await.map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let stdout = self.exec_ok(args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle {
            id,
            name: spec.name.clone(),
        })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let stdout = self.exec_ok(args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle {
            id,
            name: spec.name.clone(),
        })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = self.protocol.start_args(id);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = self.protocol.stop_args(id, timeout);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_args(id, force);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = self.protocol.list_args(all);
        let stdout = self.exec_ok(args).await?;
        self.protocol.parse_list_output(&stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = self.protocol.inspect_args(id);
        let stdout = self.exec_ok(args).await?;
        self.protocol.parse_inspect_output(&stdout)
    }

    async fn wait(&self, id: &str) -> Result<i32> {
        let args = self.protocol.wait_args(id);
        let out = self.exec_raw(args).await?;
        out.stdout.trim().parse::<i32>().map_err(|e| {
            ComposeError::BackendError {
                code: -1,
                message: format!("Failed to parse wait output: {}", e),
            }
        })
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        let args = self.protocol.inspect_image_args(reference);
        let stdout = self.exec_ok(args).await?;
        self.protocol.parse_inspect_image_output(&stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = self.protocol.logs_args(id, tail);
        let out = self.exec_raw(args).await?;
        Ok(ContainerLogs {
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let args = self.protocol.exec_args(id, cmd, env, workdir);
        let out = self.exec_raw(args).await?;
        Ok(ContainerLogs {
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }

    async fn build(
        &self,
        spec: &crate::types::ComposeServiceBuild,
        image_name: &str,
    ) -> Result<()> {
        let args = self.protocol.build_args(spec, image_name);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = self.protocol.pull_image_args(reference);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = self.protocol.list_images_args();
        let stdout = self.exec_ok(args).await?;
        self.protocol.parse_list_images_output(&stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_image_args(reference, force);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_network_args(name);
        match self.exec_ok(args).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("not found") {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_ok(args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        match self.exec_ok(args).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("not found") {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn inspect_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.inspect_network_args(name);
        self.exec_ok(args).await.map(|_| ())
    }
}

/// Detect the available container backend.
pub async fn detect_backend() -> std::result::Result<Arc<dyn ContainerBackend + Send + Sync>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await.map_err(|reason| {
            vec![BackendProbeResult {
                name,
                available: false,
                reason,
            }]
        });
    }

    let candidates = platform_candidates();
    let mut results = Vec::new();

    for candidate in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult {
                name: candidate.to_string(),
                available: false,
                reason,
            }),
            Err(_) => results.push(BackendProbeResult {
                name: candidate.to_string(),
                available: false,
                reason: "probe timed out".to_string(),
            }),
        }
    }

    Err(results)
}

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &[
            "apple/container",
            "orbstack",
            "colima",
            "rancher-desktop",
            "lima",
            "podman",
            "nerdctl",
            "docker",
        ]
    } else if cfg!(target_os = "linux") {
        &["podman", "nerdctl", "docker"]
    } else {
        &["podman", "nerdctl", "docker"]
    }
}

async fn probe_candidate(name: &str) -> std::result::Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|_| "binary not found".to_string())?;
            let backend = CliBackend::new(bin, AppleContainerProtocol);
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "binary not found".to_string())?;
            if cfg!(target_os = "macos") {
                check_podman_machine_running(&bin).await?;
            }
            let backend = CliBackend::new(bin, DockerProtocol);
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| "binary not found".to_string())?;
            let backend = CliBackend::new(bin, DockerProtocol);
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        "orbstack" => {
            let bin = which::which("orb")
                .or_else(|_| which::which("docker"))
                .map_err(|_| "binary not found".to_string())?;
            check_orbstack_socket_or_version(&bin).await?;
            let backend = CliBackend::new(bin, DockerProtocol);
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|_| "binary not found".to_string())?;
            let backend = CliBackend::new(bin, DockerProtocol);
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "binary not found".to_string())?;
            let instance = check_lima_running_instance(&bin).await?;
            let backend = CliBackend::new(bin, LimaProtocol { instance });
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        "colima" => {
            let bin = which::which("colima").map_err(|_| "binary not found".to_string())?;
            check_colima_running(&bin).await?;
            let docker_bin = which::which("docker").map_err(|_| "docker binary not found".to_string())?;
            let backend = CliBackend::new(docker_bin, DockerProtocol);
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl binary not found".to_string())?;
            check_rancher_socket().await?;
            let backend = CliBackend::new(bin, DockerProtocol);
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(backend))
        }
        _ => Err("unknown backend".into()),
    }
}

async fn check_podman_machine_running(bin: &Path) -> std::result::Result<(), String> {
    let out = tokio::process::Command::new(bin)
        .args(["machine", "list", "--format", "json"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.contains("\"Running\":true") || stdout.contains("\"Running\": true") {
        Ok(())
    } else {
        Err("no running podman machine found".to_string())
    }
}

async fn check_orbstack_socket_or_version(bin: &Path) -> std::result::Result<(), String> {
    let out = tokio::process::Command::new(bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        Ok(())
    } else {
        Err("orbstack not functional".to_string())
    }
}

async fn check_lima_running_instance(bin: &Path) -> std::result::Result<String, String> {
    let out = tokio::process::Command::new(bin)
        .args(["list", "--json"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["status"] == "Running" {
                if let Some(name) = val["name"].as_str() {
                    return Ok(name.to_string());
                }
            }
        }
    }
    Err("no running lima instance found".to_string())
}

async fn check_colima_running(bin: &Path) -> std::result::Result<(), String> {
    let out = tokio::process::Command::new(bin)
        .arg("status")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.contains("running") {
        Ok(())
    } else {
        Err("colima not running".to_string())
    }
}

async fn check_rancher_socket() -> std::result::Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let socket = PathBuf::from(home).join(".rd/run/containerd-shim.sock");
    if socket.exists() {
        Ok(())
    } else {
        Err("rancher desktop socket not found".to_string())
    }
}
