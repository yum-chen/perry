use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use which::which;

pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo};

/// Layer 1: Abstract Operations
/// Minimal network creation config — driver and labels only.
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

#[async_trait]
pub trait ContainerBackend: Send + Sync + std::fmt::Debug {
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

/// Layer 2: CLI Protocol
pub trait CliProtocol: Send + Sync + std::fmt::Debug {
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
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
        args.push(name.into());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        args.push(name.into());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        serde_json::from_str(stdout).unwrap_or_default()
    }
    fn parse_inspect_output(&self, _id: &str, stdout: &str) -> Option<ContainerInfo> {
        let v: Vec<ContainerInfo> = serde_json::from_str(stdout).unwrap_or_default();
        v.into_iter().next()
    }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        serde_json::from_str(stdout).unwrap_or_default()
    }
    fn parse_container_id(&self, stdout: &str) -> String { stdout.trim().to_string() }
}

fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args = vec!["run".to_string()];
    if include_detach { args.push("-d".into()); }
    if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
    if let Some(ports) = &spec.ports { for p in ports { args.extend(["-p".into(), p.clone()]); } }
    if let Some(volumes) = &spec.volumes { for v in volumes { args.extend(["-v".into(), v.clone()]); } }
    if let Some(env) = &spec.env { for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
    if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
    if let Some(labels) = &spec.labels { for (k, v) in labels { args.extend(["--label".into(), format!("{}={}", k, v)]); } }
    if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
    if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
    if let Some(entrypoint) = &spec.entrypoint { args.extend(["--entrypoint".into(), entrypoint.join(" ")]); }
    args.push(spec.image.clone());
    if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
    args
}

#[derive(Debug)]
pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker-compatible" }
}

#[derive(Debug)]
pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, false)
    }
}

#[derive(Debug)]
pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

/// Layer 3: Generic CLI Executor
#[derive(Debug)]
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
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
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
    async fn start(&self, id: &str) -> Result<()> { self.exec_ok(self.protocol.start_args(id)).await?; Ok(()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.exec_ok(self.protocol.stop_args(id, timeout)).await?; Ok(()) }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_args(id, force)).await?; Ok(()) }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let stdout = self.exec_ok(self.protocol.list_args(all)).await?;
        Ok(self.protocol.parse_list_output(&stdout))
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let stdout = self.exec_ok(self.protocol.inspect_args(id)).await?;
        self.protocol.parse_inspect_output(id, &stdout).ok_or_else(|| ComposeError::NotFound(id.into()))
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_ok(self.protocol.pull_image_args(reference)).await?; Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let stdout = self.exec_ok(self.protocol.list_images_args()).await?;
        Ok(self.protocol.parse_list_images_output(&stdout))
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_image_args(reference, force)).await?; Ok(()) }
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> { self.exec_ok(self.protocol.create_network_args(name, config)).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_network_args(name)).await?; Ok(()) }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> { self.exec_ok(self.protocol.create_volume_args(name, config)).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_volume_args(name)).await?; Ok(()) }
}

/// Layer 4: Runtime Detection
pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend + Send + Sync>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map_err(|reason| vec![BackendProbeResult { name: name.clone(), available: false, reason }]);
    }

    let candidates = get_platform_candidates();

    let mut results = Vec::new();
    for candidate in candidates {
        match timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: "probe timed out".into() }),
        }
    }
    Err(results)
}

pub async fn probe_all_candidates() -> Vec<BackendProbeResult> {
    let candidates = get_platform_candidates();
    let mut results = Vec::new();

    for candidate in candidates {
        match timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => results.push(BackendProbeResult {
                name: candidate.to_string(),
                available: true,
                reason: format!("Found at {:?}", backend.backend_name()),
            }),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: "probe timed out".into() }),
        }
    }
    results
}

fn get_platform_candidates() -> &'static [&'static str] {
    match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    }
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend + Send + Sync>, String> {
    match name {
        "apple/container" => {
            let bin = which("container").map_err(|_| "binary not found".to_string())?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which("podman").map_err(|_| "binary not found".to_string())?;
            if std::env::consts::OS == "macos" {
                let output = Command::new(&bin).args(["machine", "list", "--format", "json"]).output().await.map_err(|e| e.to_string())?;
                let val: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
                if !val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true))) {
                    return Err("no running podman machine".into());
                }
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker")).map_err(|_| "orbstack not found".to_string())?;
            let socket = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
            if !socket.map_or(false, |s| s.exists()) {
                return Err("OrbStack socket not found".into());
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "colima" => {
            let bin = which("colima").map_err(|_| "colima not found".to_string())?;
            let output = Command::new(&bin).arg("status").output().await.map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&output.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let docker_bin = which("docker").map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(Box::new(CliBackend::new(docker_bin, DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl not found".to_string())?;
            let socket = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
            if !socket.map_or(false, |s| s.exists()) {
                return Err("Rancher Desktop socket not found".into());
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which("limactl").map_err(|_| "limactl not found".to_string())?;
            let output = Command::new(&bin).args(["list", "--json"]).output().await.map_err(|e| e.to_string())?;
            let mut instance = None;
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    if val["status"].as_str() == Some("Running") {
                        instance = val["name"].as_str().map(|s| s.to_string());
                        break;
                    }
                }
            }
            let instance = instance.ok_or_else(|| "no running lima instance".to_string())?;
            Ok(Box::new(CliBackend::new(bin, LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl not found".to_string())?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which("docker").map_err(|_| "docker not found".to_string())?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        _ => Err("unknown backend".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_backend_respects_env() {
        std::env::set_var("PERRY_CONTAINER_BACKEND", "nonexistent");
        let result = detect_backend().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].name, "nonexistent");
        std::env::remove_var("PERRY_CONTAINER_BACKEND");
    }
}
