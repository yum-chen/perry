use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde_json::Value;
pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, NetworkConfig, VolumeConfig, ComposeServiceBuild};

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
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
    async fn inspect_network(&self, name: &str) -> Result<()>;
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
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
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
    fn inspect_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "inspect".into(), name.into()] }

    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> {
        let mut args = vec!["build".into()];
        if let Some(dockerfile) = &spec.dockerfile { args.extend(["-f".into(), dockerfile.clone()]); }
        if let Some(build_args) = &spec.args {
            for (k, v) in build_args.to_map() {
                args.extend(["--build-arg".into(), format!("{}={}", k, v)]);
            }
        }
        args.extend(["-t".into(), image_name.into()]);
        args.push(spec.context.clone().unwrap_or_else(|| ".".into()));
        args
    }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        if let Ok(v) = serde_json::from_str::<Value>(stdout) {
            if let Some(arr) = v.as_array() {
                return arr.iter().filter_map(|c| parse_container_info_from_json(c).ok()).collect();
            }
        }
        Vec::new()
    }
    fn parse_inspect_output(&self, _id: &str, stdout: &str) -> Option<ContainerInfo> {
        if let Ok(v) = serde_json::from_str::<Value>(stdout) {
            if let Some(arr) = v.as_array() {
                if let Some(first) = arr.first() {
                    return parse_container_info_from_json(first).ok();
                }
            } else {
                return parse_container_info_from_json(&v).ok();
            }
        }
        None
    }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        if let Ok(v) = serde_json::from_str::<Value>(stdout) {
            if let Some(arr) = v.as_array() {
                return arr.iter().filter_map(|i| parse_image_info_from_json(i).ok()).collect();
            }
        }
        Vec::new()
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
    if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
    if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
    if let Some(entrypoint) = &spec.entrypoint { args.extend(["--entrypoint".into(), entrypoint.join(" ")]); }
    args.push(spec.image.clone());
    if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
    args
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
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> {
        let mut args = vec!["build".into()];
        if let Some(dockerfile) = &spec.dockerfile { args.extend(["-f".into(), dockerfile.clone()]); }
        args.extend([
            "--cpus".into(), "2".into(),
            "--memory".into(), "2048MB".into(),
            "--arch".into(), "arm64".into(),
            "--os".into(), "linux".into(),
            "-t".into(), image_name.into(),
        ]);
        args.push(spec.context.clone().unwrap_or_else(|| ".".into()));
        args
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

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self { Self { bin, protocol } }

    async fn exec_raw(&self, subcommand_args: Vec<String>) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(subcommand_args);
        cmd.output().await.map_err(ComposeError::IoError)
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
    fn backend_name(&self) -> &str { self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown") }
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
        Ok(ContainerHandle { id: self.protocol.parse_container_id(&stdout), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.create_args(spec)).await?;
        Ok(ContainerHandle { id: self.protocol.parse_container_id(&stdout), name: spec.name.clone() })
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
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.exec_ok(self.protocol.build_args(spec, image_name)).await?;
        Ok(())
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_ok(self.protocol.pull_image_args(reference)).await?; Ok(()) }
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
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_network_args(name)).await?; Ok(()) }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_volume_args(name, config)).await?;
        Ok(())
    }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_volume_args(name)).await?; Ok(()) }
    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.exec_ok(self.protocol.inspect_network_args(name)).await?;
        Ok(())
    }
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
        return match probe_candidate(&name).await {
            Ok(backend) => Ok(backend),
            Err(reason) => Err(vec![BackendProbeResult { name, available: false, reason }]),
        };
    }

    let candidates: &[&str] = match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "lima", "podman", "nerdctl", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    };

    let mut results = Vec::new();
    for &name in candidates {
        match timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: name.into(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: name.into(), available: false, reason: "timed out".into() }),
        }
    }
    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend + Send + Sync>, String> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|_| "container not found")?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "podman not found")?;
            if std::env::consts::OS == "macos" {
                let mut cmd = Command::new(&bin);
                cmd.args(["machine", "list", "--format", "json"]);
                let output = cmd.output().await.map_err(|e| e.to_string())?;
                let val: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
                if !val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true))) {
                    return Err("no running podman machine".into());
                }
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "orbstack" => {
            let bin = which::which("orb").or_else(|_| which::which("docker")).map_err(|_| "orb or docker not found")?;
            let socket = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
            if socket.map_or(false, |s| s.exists()) {
                Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
            } else {
                Err("orbstack socket not found".into())
            }
        }
        "colima" => {
            let bin = which::which("colima").map_err(|_| "colima not found")?;
            let mut cmd = Command::new(&bin);
            cmd.arg("status");
            let output = cmd.output().await.map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&output.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let docker_bin = which::which("docker").map_err(|_| "docker not found")?;
            Ok(Box::new(CliBackend::new(docker_bin, DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl not found")?;
            let socket = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
            if socket.map_or(false, |s| s.exists()) {
                Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
            } else {
                Err("rancher desktop socket not found".into())
            }
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "limactl not found")?;
            let mut cmd = Command::new(&bin);
            cmd.args(["list", "--json"]);
            let output = cmd.output().await.map_err(|e| e.to_string())?;
            let mut instance = None;
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if let Ok(val) = serde_json::from_str::<Value>(line) {
                    if val["status"].as_str() == Some("Running") {
                        instance = val["name"].as_str().map(|s| s.to_string());
                        break;
                    }
                }
            }
            if let Some(inst) = instance {
                let nerdctl_bin = which::which("nerdctl").unwrap_or_else(|_| bin.clone());
                Ok(Box::new(CliBackend::new(nerdctl_bin, LimaProtocol { instance: inst })))
            } else {
                Err("no running lima instance".into())
            }
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl not found")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| "docker not found")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        _ => Err("unknown backend".into())
    }
}

pub struct MockBackend;
#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
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
    async fn build(&self, _spec: &ComposeServiceBuild, _image_name: &str) -> Result<()> { Ok(()) }
    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, _name: &str, _config: &NetworkConfig) -> Result<()> { Ok(()) }
    async fn remove_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, _name: &str, _config: &VolumeConfig) -> Result<()> { Ok(()) }
    async fn remove_volume(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn inspect_network(&self, _name: &str) -> Result<()> { Ok(()) }
}
