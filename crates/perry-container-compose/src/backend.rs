use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde::{Deserialize, Serialize};
use which::which;
pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo};

/// Minimal network creation config — driver and labels only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

/// Minimal volume creation config — driver and labels only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
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
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs>;
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
    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
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
        if all { args.push("-a".into()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(n) = tail { args.extend(["--tail".into(), n.to_string()]); }
        args.push(id.into());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env { for (k, v) in e { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(w) = workdir { args.extend(["-w".into(), w.into()]); }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { vec!["pull".into(), reference.into()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".into(), "--format".into(), "json".into()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        if config.internal { args.push("--internal".into()); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        args.push(name.into());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        args.push(name.into());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        let mut result = Vec::new();
        for line in stdout.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if let Ok(info) = parse_container_info_from_json(&v) {
                    result.push(info);
                }
            }
        }
        result
    }
    fn parse_inspect_output(&self, _id: &str, stdout: &str) -> Option<ContainerInfo> {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(stdout) {
             let first = if v.is_array() { v.as_array().and_then(|a| a.first()).unwrap_or(&v) } else { &v };
             return parse_container_info_from_json(first).ok();
        }
        None
    }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        let mut result = Vec::new();
        for line in stdout.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if let Ok(info) = parse_image_info_from_json(&v) {
                    result.push(info);
                }
            }
        }
        result
    }
    fn parse_container_id(&self, stdout: &str) -> String { stdout.trim().to_string() }
}

pub fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args = if include_detach { vec!["run".into(), "-d".into()] } else { vec!["run".into()] };
    if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
    if let Some(ports) = &spec.ports { for p in ports { args.extend(["-p".into(), p.clone()]); } }
    if let Some(volumes) = &spec.volumes { for v in volumes { args.extend(["-v".into(), v.clone()]); } }
    if let Some(env) = &spec.env { for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
    if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
    if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
    if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
    if let Some(entrypoint) = &spec.entrypoint { args.extend(["--entrypoint".into(), entrypoint.join(" ")]); }
    args.push(spec.image.clone());
    if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
    args
}

fn parse_container_info_from_json(json: &serde_json::Value) -> Result<ContainerInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
    let name = json["Names"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str())
        .or(json["Name"].as_str())
        .unwrap_or("")
        .trim_start_matches('/')
        .to_string();
    let image = json["Image"].as_str().unwrap_or("").to_string();
    let status = json["Status"].as_str().or_else(|| json["State"].get("Status").and_then(|v| v.as_str())).unwrap_or("").to_string();
    Ok(ContainerInfo { id, name, image, status, ports: Vec::new(), created: json["Created"].as_str().unwrap_or("").to_string() })
}

fn parse_image_info_from_json(json: &serde_json::Value) -> Result<ImageInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
    Ok(ImageInfo { id, repository: json["Repository"].as_str().unwrap_or("").to_string(), tag: json["Tag"].as_str().unwrap_or("").to_string(), size: json["Size"].as_u64().unwrap_or(0), created: json["Created"].as_str().unwrap_or("").to_string() })
}

pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker-compatible" }
}

pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, false)
    }
}

pub struct LimaProtocol { pub instance: String }
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
    pub fn new(bin: PathBuf, protocol: P) -> Self { Self { bin, protocol } }

    async fn exec_raw(&self, subcommand_args: Vec<String>) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(subcommand_args);
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        Ok(output)
    }

    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let output = self.exec_raw(args).await?;
        if !output.status.success() {
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl<P: CliProtocol + Send + Sync> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        let output = self.exec_raw(vec!["--version".into()]).await?;
        if !output.status.success() {
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(1),
                message: "Backend --version check failed".into(),
            });
        }
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.run_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.create_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.exec_ok(self.protocol.start_args(id)).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.exec_ok(self.protocol.stop_args(id, timeout)).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.exec_ok(self.protocol.remove_args(id, force)).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let stdout = self.exec_ok(self.protocol.list_args(all)).await?;
        Ok(self.protocol.parse_list_output(&stdout))
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let stdout = self.exec_ok(self.protocol.inspect_args(id)).await?;
        self.protocol.parse_inspect_output(id, &stdout).ok_or_else(|| ComposeError::NotFound(id.into()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() { cmd.args(prefix); }
        cmd.args(self.protocol.logs_args(id, tail));
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut command = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() { command.args(prefix); }
        command.args(self.protocol.exec_args(id, cmd, env, workdir));
        let output = command.output().await.map_err(ComposeError::IoError)?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.exec_ok(self.protocol.pull_image_args(reference)).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let stdout = self.exec_ok(self.protocol.list_images_args()).await?;
        Ok(self.protocol.parse_list_images_output(&stdout))
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.exec_ok(self.protocol.remove_image_args(reference, force)).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_network_args(name, config)).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.exec_ok(self.protocol.remove_network_args(name)).await?;
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_volume_args(name, config)).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.exec_ok(self.protocol.remove_volume_args(name)).await?;
        Ok(())
    }
}

pub async fn detect_backend() -> std::result::Result<Arc<dyn ContainerBackend + Send + Sync>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map_err(|reason| vec![BackendProbeResult {
                name: name.clone(), available: false, reason,
            }]);
    }

    let candidates: &[&str] = match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        "linux" => &["podman", "nerdctl", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    };
    let mut results = Vec::new();

    for candidate in candidates {
        match timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult {
                name: candidate.to_string(), available: false,
                reason: "probe timed out after 2s".to_string(),
            }),
        }
    }

    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    match name {
        "apple/container" => {
            let bin = which("container").map_err(|_| "container binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            Ok(Arc::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which("podman").map_err(|_| "podman binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            if cfg!(target_os = "macos") { check_podman_machine_running(&bin).await?; }
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker"))
                .map_err(|_| "orbstack not found".to_string())?;
            check_orbstack_socket_or_version(&bin).await?;
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        "colima" => {
            let bin = which("colima").map_err(|_| "colima binary not found on PATH".to_string())?;
            check_colima_running(&bin).await?;
            let docker_bin = which("docker").map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(Arc::new(CliBackend::new(docker_bin, DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            check_rancher_socket().await?;
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which("limactl").map_err(|_| "limactl binary not found on PATH".to_string())?;
            let instance = check_lima_running_instance(&bin).await?;
            Ok(Arc::new(CliBackend::new(bin, LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which("docker").map_err(|_| "docker binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        other => Err(format!("unknown backend: {other}")),
    }
}

async fn run_version_check(bin: &std::path::Path) -> std::result::Result<(), String> {
    let mut cmd = Command::new(bin);
    cmd.arg("--version");
    let res = timeout(Duration::from_secs(2), cmd.output()).await;
    if matches!(res, Ok(Ok(ref output)) if output.status.success()) {
        Ok(())
    } else {
        Err("CLI version check failed".to_string())
    }
}

async fn check_podman_machine_running(bin: &std::path::Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).args(["machine", "list", "--format", "json"]).output().await.map_err(|e| e.to_string())?;
    let val: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    if val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true))) {
        Ok(())
    } else {
        Err("no running podman machine".to_string())
    }
}

async fn check_orbstack_socket_or_version(bin: &std::path::Path) -> std::result::Result<(), String> {
    if let Some(h) = home::home_dir() {
        if h.join(".orbstack/run/docker.sock").exists() { return Ok(()); }
    }
    run_version_check(bin).await
}

async fn check_colima_running(bin: &std::path::Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).arg("status").output().await.map_err(|e| e.to_string())?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("running") {
        Ok(())
    } else {
        Err("colima status not running".into())
    }
}

async fn check_rancher_socket() -> std::result::Result<(), String> {
    if let Some(h) = home::home_dir() {
        if h.join(".rd/run/containerd-shim.sock").exists() { return Ok(()); }
    }
    Err("Rancher Desktop socket not found".into())
}

async fn check_lima_running_instance(bin: &std::path::Path) -> std::result::Result<String, String> {
    let output = Command::new(bin).args(["list", "--json"]).output().await.map_err(|e| e.to_string())?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["status"].as_str() == Some("Running") {
                return Ok(val["name"].as_str().unwrap_or("default").to_string());
            }
        }
    }
    Err("no running lima instance".into())
}
