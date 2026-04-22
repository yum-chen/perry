use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde_json::Value;
pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ComposeNetwork, ComposeVolume};

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;
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
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Podman { bin: PathBuf },
    OrbStack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf },
    Lima { bin: PathBuf },
    Nerdctl { bin: PathBuf },
    Docker { bin: PathBuf },
}

impl BackendDriver {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AppleContainer { .. } => "apple/container",
            Self::Podman { .. } => "podman",
            Self::OrbStack { .. } => "orbstack",
            Self::Colima { .. } => "colima",
            Self::RancherDesktop { .. } => "rancher-desktop",
            Self::Lima { .. } => "lima",
            Self::Nerdctl { .. } => "nerdctl",
            Self::Docker { .. } => "docker",
        }
    }
    pub fn bin(&self) -> &Path {
        match self {
            Self::AppleContainer { bin } => bin,
            Self::Podman { bin } => bin,
            Self::OrbStack { bin } => bin,
            Self::Colima { bin } => bin,
            Self::RancherDesktop { bin } => bin,
            Self::Lima { bin } => bin,
            Self::Nerdctl { bin } => bin,
            Self::Docker { bin } => bin,
        }
    }
    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }
}

pub struct OciCommandBuilder;
impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        if driver.is_docker_compatible() {
            Self::docker_run_args(spec)
        } else {
            match driver {
                BackendDriver::AppleContainer { .. } => Self::apple_run_args(spec),
                BackendDriver::Lima { .. } => Self::lima_run_args(spec),
                _ => unreachable!(),
            }
        }
    }
    fn docker_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
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
    fn apple_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        args
    }
    fn lima_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), "default".into(), "nerdctl".into(), "run".into(), "-d".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        args
    }
}

pub struct OciBackend { pub driver: BackendDriver }
impl OciBackend {
    pub fn new(driver: BackendDriver) -> Self { Self { driver } }
    async fn exec_cli(&self, args: &[String]) -> Result<std::process::Output> {
        let mut cmd = Command::new(self.driver.bin());
        cmd.args(args);
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        if !output.status.success() {
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(output)
    }
}
#[async_trait]
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &'static str { self.driver.name() }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(self.driver.bin());
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec);
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> { self.exec_cli(&["start".into(), id.into()]).await?; Ok(()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        self.exec_cli(&args).await?;
        Ok(())
    }
    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        self.exec_cli(&args).await?;
        Ok(())
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("-a".into()); }
        let output = self.exec_cli(&args).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for c in arr { result.push(parse_container_info_from_json(c)?); }
        }
        Ok(result)
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self.exec_cli(&["inspect".into(), "--format".into(), "json".into(), id.into()]).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let first = v.as_array().and_then(|a| a.first()).ok_or_else(|| ComposeError::NotFound(id.into()))?;
        parse_container_info_from_json(first)
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut args = vec!["logs".into()];
        if let Some(n) = tail { args.extend(["--tail".into(), n.to_string()]); }
        args.push(id.into());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env { for (k, v) in e { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(w) = workdir { args.extend(["-w".into(), w.into()]); }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_cli(&["pull".into(), reference.into()]).await?; Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = self.exec_cli(&["images".into(), "--format".into(), "json".into()]).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for i in arr { result.push(parse_image_info_from_json(i)?); }
        }
        Ok(result)
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        self.exec_cli(&args).await?;
        Ok(())
    }
    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> { self.exec_cli(&["network".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_cli(&["network".into(), "rm".into(), name.into()]).await?; Ok(()) }
    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> { self.exec_cli(&["volume".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_cli(&["volume".into(), "rm".into(), name.into()]).await?; Ok(()) }
}

fn parse_container_info_from_json(json: &Value) -> Result<ContainerInfo> {
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

fn parse_image_info_from_json(json: &Value) -> Result<ImageInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
    Ok(ImageInfo { id, repository: json["Repository"].as_str().unwrap_or("").to_string(), tag: json["Tag"].as_str().unwrap_or("").to_string(), size: json["Size"].as_u64().unwrap_or(0), created: json["Created"].as_str().unwrap_or("").to_string() })
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend + Send + Sync>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let res = probe_candidate(&name).await;
        if res.available { return Ok(make_backend(&name, PathBuf::from(res.reason))); }
        return Err(vec![res]);
    }
    let candidates: &[&str] = match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        "linux" => &["podman", "nerdctl", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    };
    let mut results = Vec::new();
    for &name in candidates {
        let result = probe_candidate(name).await;
        if result.available { return Ok(make_backend(name, PathBuf::from(result.reason))); }
        results.push(result);
    }
    Err(results)
}

async fn probe_candidate(name: &str) -> BackendProbeResult {
    let check = match name {
        "apple/container" => ("container", vec!["--version"]),
        "orbstack" => ("orb", vec!["--version"]),
        "colima" => ("colima", vec!["--version"]),
        "rancher-desktop" => ("nerdctl", vec!["--version"]),
        "podman" => ("podman", vec!["--version"]),
        "lima" => ("limactl", vec!["--version"]),
        "nerdctl" => ("nerdctl", vec!["--version"]),
        "docker" => ("docker", vec!["--version"]),
        _ => return BackendProbeResult { name: name.into(), available: false, reason: "unknown candidate".into() },
    };
    let bin = match which::which(check.0) {
        Ok(p) => p,
        Err(_) => return BackendProbeResult { name: name.into(), available: false, reason: format!("{} not found", check.0) },
    };

    let mut cmd = Command::new(&bin);
    cmd.args(check.1);
    let probe_res = timeout(Duration::from_secs(2), cmd.output()).await;
    if !matches!(probe_res, Ok(Ok(ref output)) if output.status.success()) {
         return BackendProbeResult { name: name.into(), available: false, reason: "CLI check failed".into() };
    }

    match name {
        "orbstack" => {
            let socket: Option<PathBuf> = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
            if socket.map_or(false, |s: PathBuf| s.exists()) {
                BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
            } else {
                BackendProbeResult { name: name.into(), available: false, reason: "OrbStack socket not found".into() }
            }
        }
        "colima" => {
            let mut cmd = Command::new(&bin);
            cmd.arg("status");
            let res = timeout(Duration::from_secs(2), cmd.output()).await;
            if matches!(res, Ok(Ok(ref output)) if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("running")) {
                BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
            } else {
                BackendProbeResult { name: name.into(), available: false, reason: "colima status not running".into() }
            }
        }
        "rancher-desktop" => {
             let socket: Option<PathBuf> = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
             if socket.map_or(false, |s: PathBuf| s.exists()) {
                 BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
             } else {
                 BackendProbeResult { name: name.into(), available: false, reason: "Rancher Desktop socket not found".into() }
             }
        }
        "podman" if std::env::consts::OS == "macos" => {
             let mut cmd = Command::new(&bin);
             cmd.args(["machine", "list", "--format", "json"]);
             let res = timeout(Duration::from_secs(2), cmd.output()).await;
             if let Ok(Ok(output)) = res {
                 if let Ok(val) = serde_json::from_slice::<Value>(&output.stdout) {
                     if val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true))) {
                         return BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() };
                     }
                 }
             }
             BackendProbeResult { name: name.into(), available: false, reason: "no running podman machine".into() }
        }
        "lima" => {
             let mut cmd = Command::new(&bin);
             cmd.args(["list", "--json"]);
             let res = timeout(Duration::from_secs(2), cmd.output()).await;
             if let Ok(Ok(output)) = res {
                 for line in String::from_utf8_lossy(&output.stdout).lines() {
                     if let Ok(val) = serde_json::from_str::<Value>(line) {
                         if val["status"].as_str() == Some("Running") {
                             return BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() };
                         }
                     }
                 }
             }
             BackendProbeResult { name: name.into(), available: false, reason: "no running lima instance".into() }
        }
        _ => BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
    }
}

fn make_backend(name: &str, bin: PathBuf) -> Box<dyn ContainerBackend + Send + Sync> {
    let driver = match name {
        "apple/container" => BackendDriver::AppleContainer { bin },
        "podman" => BackendDriver::Podman { bin },
        "orbstack" => BackendDriver::OrbStack { bin },
        "colima" => BackendDriver::Colima { bin },
        "rancher-desktop" => BackendDriver::RancherDesktop { bin },
        "lima" => BackendDriver::Lima { bin },
        "nerdctl" => BackendDriver::Nerdctl { bin },
        "docker" => BackendDriver::Docker { bin },
        _ => unreachable!(),
    };
    Box::new(OciBackend::new(driver))
}

pub struct MockBackend;
#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &'static str { "mock" }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "mock".into(), name: None }) }
    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "mock".into(), name: None }) }
    async fn start(&self, _id: &str) -> Result<()> { Ok(()) }
    async fn stop(&self, _id: &str, _timeout: Option<u32>) -> Result<()> { Ok(()) }
    async fn remove(&self, _id: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> { Ok(vec![]) }
    async fn inspect(&self, _id: &str) -> Result<ContainerInfo> { Err(ComposeError::NotFound("mock".into())) }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, _name: &str, _config: &ComposeNetwork) -> Result<()> { Ok(()) }
    async fn remove_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, _name: &str, _config: &ComposeVolume) -> Result<()> { Ok(()) }
    async fn remove_volume(&self, _name: &str) -> Result<()> { Ok(()) }
}
