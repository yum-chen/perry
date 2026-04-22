//! Container backend abstraction — `ContainerBackend` trait, `CliProtocol` trait,
//! protocol implementations (`DockerProtocol`, `AppleContainerProtocol`, `LimaProtocol`),
//! generic `CliBackend<P>`, and `detect_backend()`.

use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

// ─────────────────────────────────────────────────────────────────────────────
// 4.8  BackendProbeResult — defined in error.rs, re-exported here
// ─────────────────────────────────────────────────────────────────────────────
pub use crate::error::BackendProbeResult;

// ─────────────────────────────────────────────────────────────────────────────
// 4.1  NetworkConfig and VolumeConfig — lean config structs
// ─────────────────────────────────────────────────────────────────────────────

/// Lean network configuration decoupled from compose-spec types.
#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

/// Lean volume configuration decoupled from compose-spec types.
#[derive(Debug, Clone, Default)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
}

/// Execution strategy for a container backend.
#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    CliExec { bin: PathBuf },
    ApiSocket { socket: PathBuf },
    VmSpawn { config: serde_json::Value },
}

/// Security profile for sandboxed OCI containers.
#[derive(Debug, Clone, Default)]
pub struct SecurityProfile {
    pub read_only_rootfs: bool,
    pub seccomp_profile: Option<String>,
    pub cap_drop: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversions from compose-spec types to lean config types
// ─────────────────────────────────────────────────────────────────────────────

impl From<&ComposeNetwork> for NetworkConfig {
    fn from(n: &ComposeNetwork) -> Self {
        NetworkConfig {
            driver: n.driver.clone(),
            labels: n.labels.as_ref().map(|l| l.to_map()).unwrap_or_default(),
            internal: n.internal.unwrap_or(false),
            enable_ipv6: n.enable_ipv6.unwrap_or(false),
        }
    }
}

impl From<&ComposeVolume> for VolumeConfig {
    fn from(v: &ComposeVolume) -> Self {
        VolumeConfig {
            driver: v.driver.clone(),
            labels: v.labels.as_ref().map(|l| l.to_map()).unwrap_or_default(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.1  ContainerBackend trait
// ─────────────────────────────────────────────────────────────────────────────

/// Protocol-specific argument builder for container CLI runtimes.
pub trait CliProtocol: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String>;
    fn pull_image_args(&self, reference: &str) -> Vec<String>;
    fn build_args(
        &self,
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String>;
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String>;
    fn wait_args(&self, id: &str) -> Vec<String>;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }
}

/// Runtime-agnostic async interface for container operations.
/// Follows canonical method set from SPEC.md §2.2.
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
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
    async fn inspect_network(&self, name: &str) -> Result<serde_json::Value>;
    async fn inspect_volume(&self, name: &str) -> Result<serde_json::Value>;

    async fn build_image(
        &self,
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Result<()>;

    async fn inspect_image(&self, reference: &str) -> Result<serde_json::Value>;
    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value>;

    /// Run with enhanced security constraints (seccomp, read-only fs).
    async fn run_with_security(
        &self,
        spec: &ContainerSpec,
        profile: &SecurityProfile,
    ) -> Result<ContainerHandle>;

    /// Wait for a container to exit and collect its logs.
    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs>;

    /// Get execution strategy for this backend.
    fn strategy(&self) -> ExecutionStrategy;

    /// Get isolation level provided by this backend.
    fn isolation_level(&self) -> crate::types::IsolationLevel;
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared JSON deserialization helpers (Docker-compatible output format)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DockerListEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Names", alias = "names", default)]
    names: serde_json::Value,
    #[serde(rename = "Image", alias = "image", default)]
    image: String,
    #[serde(rename = "Status", alias = "status", default)]
    status: String,
    #[serde(rename = "Ports", alias = "ports", default)]
    ports: serde_json::Value,
    #[serde(rename = "Labels", alias = "labels", default)]
    labels: serde_json::Value,
    #[serde(rename = "Created", alias = "created", default)]
    created: serde_json::Value,
}

impl DockerListEntry {
    fn into_container_info(self) -> ContainerInfo {
        let labels = match self.labels {
            serde_json::Value::Object(map) => map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                .collect(),
            serde_json::Value::String(s) if !s.is_empty() => s
                .split(',')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect(),
            _ => HashMap::new(),
        };

        let name = match &self.names {
            serde_json::Value::Array(arr) => arr
                .first()
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches('/').to_string())
                .unwrap_or_default(),
            serde_json::Value::String(s) => s.trim_start_matches('/').to_string(),
            _ => String::new(),
        };
        let ports = match &self.ports {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            serde_json::Value::String(s) if !s.is_empty() => vec![s.clone()],
            _ => vec![],
        };
        let created = match &self.created {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        ContainerInfo {
            id: self.id,
            name,
            image: self.image,
            status: self.status,
            ports,
            labels,
            created,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DockerInspectEntry {
    #[serde(rename = "Id", alias = "ID", default)]
    id: String,
    #[serde(rename = "Name", alias = "name", default)]
    name: String,
    #[serde(rename = "Image", alias = "image", default)]
    image: String,
    #[serde(rename = "Config", alias = "config")]
    config: Option<DockerInspectConfig>,
    #[serde(rename = "State", alias = "state")]
    state: Option<DockerInspectState>,
    #[serde(rename = "Created", alias = "created", default)]
    created: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectConfig {
    #[serde(rename = "Labels", alias = "labels", default)]
    labels: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct DockerInspectState {
    #[serde(rename = "Running", alias = "running", default)]
    running: bool,
    #[serde(rename = "Status", alias = "status", default)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct DockerImageEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Repository", alias = "repository", default)]
    repository: String,
    #[serde(rename = "Tag", alias = "tag", default)]
    tag: String,
    #[serde(rename = "Size", alias = "size", default)]
    size: serde_json::Value,
    #[serde(rename = "Created", alias = "created", default)]
    created: String,
}

fn parse_size(v: &serde_json::Value) -> u64 {
    match v {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        serde_json::Value::String(s) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn is_not_found(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("not found")
        || s.contains("no such")
        || s.contains("does not exist")
        || s.contains("unknown container")
}

/// Build the common Docker-compatible `run`/`create` flags from a `ContainerSpec`.
/// When `include_detach` is true, `--detach` is added (Docker/podman/nerdctl).
/// When false (apple/container), it is omitted.
pub fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if spec.rm.unwrap_or(false) {
        args.push("--rm".into());
    }
    if include_detach {
        args.push("--detach".into());
    }
    if let Some(name) = &spec.name {
        args.push("--name".into());
        args.push(name.clone());
    }
    if let Some(network) = &spec.network {
        args.push("--network".into());
        args.push(network.clone());
    }
    if let Some(ports) = &spec.ports {
        for p in ports {
            args.push("-p".into());
            args.push(p.clone());
        }
    }
    if let Some(vols) = &spec.volumes {
        for v in vols {
            args.push("-v".into());
            args.push(v.clone());
        }
    }
    if let Some(envs) = &spec.env {
        let mut pairs: Vec<(&String, &String)> = envs.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            args.push("-e".into());
            args.push(format!("{}={}", k, v));
        }
    }
    if let Some(ep) = &spec.entrypoint {
        args.push("--entrypoint".into());
        args.push(ep.join(" "));
    }
    args
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.2  BackendDriver enum
// ─────────────────────────────────────────────────────────────────────────────

/// Identifies the detected container runtime and its resolved CLI binary path.
#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Podman { bin: PathBuf },
    OrbStack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf }, // uses nerdctl
    Lima { bin: PathBuf, instance: String }, // uses limactl
    Nerdctl { bin: PathBuf },
    Docker { bin: PathBuf },
}

// ── Protocols ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn name(&self) -> &'static str { "docker-compatible" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> { OciCommandBuilder::docker_run_args(spec, true) }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { OciCommandBuilder::docker_run_args(spec, false) }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> { OciCommandBuilder::stop_args(id, timeout) }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> { OciCommandBuilder::remove_args(id, force) }
    fn list_args(&self, all: bool) -> Vec<String> { OciCommandBuilder::list_args(all) }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> { OciCommandBuilder::logs_args(id, tail) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        OciCommandBuilder::exec_args(id, cmd, env, workdir)
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { OciCommandBuilder::pull_image_args(reference) }
    fn build_args(&self, context: &str, tag: &str, dockerfile: Option<&str>, args: Option<&HashMap<String, String>>) -> Vec<String> {
        OciCommandBuilder::build_args(context, tag, dockerfile, args)
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> { OciCommandBuilder::create_network_args(name, config) }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> { OciCommandBuilder::create_volume_args(name, config) }
    fn start_args(&self, id: &str) -> Vec<String> { OciCommandBuilder::start_args(id) }
    fn wait_args(&self, id: &str) -> Vec<String> { OciCommandBuilder::wait_args(id) }
}

#[derive(Debug, Clone)]
pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn name(&self) -> &'static str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> { OciCommandBuilder::apple_run_args(spec) }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        args.extend(docker_run_flags(spec, false));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> { OciCommandBuilder::stop_args(id, timeout) }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> { OciCommandBuilder::remove_args(id, force) }
    fn list_args(&self, all: bool) -> Vec<String> { OciCommandBuilder::list_args(all) }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> { OciCommandBuilder::logs_args(id, tail) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        OciCommandBuilder::exec_args(id, cmd, env, workdir)
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { OciCommandBuilder::pull_image_args(reference) }
    fn build_args(&self, context: &str, tag: &str, dockerfile: Option<&str>, args: Option<&HashMap<String, String>>) -> Vec<String> {
        OciCommandBuilder::build_args(context, tag, dockerfile, args)
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> { OciCommandBuilder::create_network_args(name, config) }
    fn start_args(&self, id: &str) -> Vec<String> { OciCommandBuilder::start_args(id) }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> { OciCommandBuilder::create_volume_args(name, config) }
    fn wait_args(&self, id: &str) -> Vec<String> { OciCommandBuilder::wait_args(id) }
}

#[derive(Debug, Clone)]
pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol {
    fn name(&self) -> &'static str { "lima" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> { OciCommandBuilder::docker_run_args(spec, true) }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { OciCommandBuilder::docker_run_args(spec, false) }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> { OciCommandBuilder::stop_args(id, timeout) }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> { OciCommandBuilder::remove_args(id, force) }
    fn list_args(&self, all: bool) -> Vec<String> { OciCommandBuilder::list_args(all) }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> { OciCommandBuilder::logs_args(id, tail) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        OciCommandBuilder::exec_args(id, cmd, env, workdir)
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { OciCommandBuilder::pull_image_args(reference) }
    fn build_args(&self, context: &str, tag: &str, dockerfile: Option<&str>, args: Option<&HashMap<String, String>>) -> Vec<String> {
        OciCommandBuilder::build_args(context, tag, dockerfile, args)
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> { OciCommandBuilder::create_network_args(name, config) }
    fn start_args(&self, id: &str) -> Vec<String> { OciCommandBuilder::start_args(id) }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> { OciCommandBuilder::create_volume_args(name, config) }
    fn wait_args(&self, id: &str) -> Vec<String> { OciCommandBuilder::wait_args(id) }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

impl BackendDriver {
    /// Returns the human-readable name used in getBackend() and PERRY_CONTAINER_BACKEND.
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

    /// Returns the resolved CLI binary path.
    pub fn bin(&self) -> &PathBuf {
        match self {
            Self::AppleContainer { bin }
            | Self::Podman { bin }
            | Self::OrbStack { bin }
            | Self::Colima { bin }
            | Self::RancherDesktop { bin }
            | Self::Lima { bin, .. }
            | Self::Nerdctl { bin }
            | Self::Docker { bin } => bin,
        }
    }

    /// Returns true if this driver accepts Docker-compatible CLI flags.
    /// All drivers except AppleContainer and Lima use Docker-compatible syntax.
    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }

    /// Optional prefix inserted before every subcommand.
    pub fn subcommand_prefix(&self) -> Option<Vec<String>> {
        match self {
            Self::Lima { instance, .. } => Some(vec![
                "shell".into(),
                instance.clone(),
                "nerdctl".into(),
            ]),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.2  OciCommandBuilder struct
// ─────────────────────────────────────────────────────────────────────────────

/// Translates abstract container operations into CLI arguments for the active BackendDriver.
pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        match driver {
            BackendDriver::AppleContainer { .. } => Self::apple_run_args(spec),
            BackendDriver::Lima { .. } => Self::docker_run_args(spec, true),
            _ if driver.is_docker_compatible() => Self::docker_run_args(spec, true),
            _ => unreachable!(),
        }
    }

    pub fn create_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        match driver {
            _ if driver.is_docker_compatible() || matches!(driver, BackendDriver::Lima { .. }) => {
                Self::docker_run_args(spec, false)
            }
            _ => {
                // Default to docker-like but without detach
                let mut args = vec!["create".into()];
                args.extend(docker_run_flags(spec, false));
                args.push(spec.image.clone());
                if let Some(cmd) = &spec.cmd {
                    args.extend(cmd.iter().cloned());
                }
                args
            }
        }
    }

    pub fn docker_run_args(spec: &ContainerSpec, detach: bool) -> Vec<String> {
        let mut args = if detach {
            vec!["run".into()]
        } else {
            vec!["create".into()]
        };
        args.extend(docker_run_flags(spec, detach));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    pub fn apple_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        args.extend(docker_run_flags(spec, false));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    pub fn start_args(id: &str) -> Vec<String> {
        vec!["start".into(), id.into()]
    }

    pub fn stop_args(id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout {
            args.push("-t".into());
            args.push(t.to_string());
        }
        args.push(id.into());
        args
    }

    pub fn remove_args(id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force {
            args.push("-f".into());
        }
        args.push(id.into());
        args
    }

    pub fn list_args(all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all {
            args.push("--all".into());
        }
        args
    }

    pub fn inspect_args(id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }

    pub fn logs_args(id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail {
            args.push("--tail".into());
            args.push(t.to_string());
        }
        args.push(id.into());
        args
    }

    pub fn exec_args(
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(wd) = workdir {
            args.push("--workdir".into());
            args.push(wd.into());
        }
        if let Some(envs) = env {
            let mut pairs: Vec<(&String, &String)> = envs.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in pairs {
                args.push("-e".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }

    pub fn pull_image_args(reference: &str) -> Vec<String> {
        vec!["pull".into(), reference.into()]
    }

    pub fn list_images_args() -> Vec<String> {
        vec!["images".into(), "--format".into(), "json".into()]
    }

    pub fn remove_image_args(reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force {
            args.push("-f".into());
        }
        args.push(reference.into());
        args
    }

    pub fn create_network_args(name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        let mut pairs: Vec<(&String, &String)> = config.labels.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            args.push("--label".into());
            args.push(format!("{}={}", k, v));
        }
        if config.internal {
            args.push("--internal".into());
        }
        if config.enable_ipv6 {
            args.push("--ipv6".into());
        }
        args.push(name.into());
        args
    }

    pub fn remove_network_args(name: &str) -> Vec<String> {
        vec!["network".into(), "rm".into(), name.into()]
    }

    pub fn create_volume_args(name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        let mut pairs: Vec<(&String, &String)> = config.labels.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            args.push("--label".into());
            args.push(format!("{}={}", k, v));
        }
        args.push(name.into());
        args
    }

    pub fn remove_volume_args(name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    pub fn inspect_network_args(name: &str) -> Vec<String> {
        vec!["network".into(), "inspect".into(), name.into()]
    }

    pub fn inspect_volume_args(name: &str) -> Vec<String> {
        vec!["volume".into(), "inspect".into(), name.into()]
    }

    pub fn build_args(
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Vec<String> {
        let mut full_args = vec!["build".into(), "-t".into(), tag.into()];
        if let Some(df) = dockerfile {
            full_args.push("-f".into());
            full_args.push(df.into());
        }
        if let Some(a) = args {
            for (k, v) in a {
                full_args.push("--build-arg".into());
                full_args.push(format!("{}={}", k, v));
            }
        }
        full_args.push(context.into());
        full_args
    }

    pub fn inspect_image_args(reference: &str) -> Vec<String> {
        vec!["image".into(), "inspect".into(), reference.into()]
    }

    pub fn manifest_inspect_args(reference: &str) -> Vec<String> {
        vec!["manifest".into(), "inspect".into(), reference.into()]
    }

    pub fn run_with_security_args(
        driver: &BackendDriver,
        spec: &ContainerSpec,
        profile: &SecurityProfile,
    ) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::AppleContainer { .. } => Self::apple_run_args(spec),
            _ => Self::docker_run_args(spec, true),
        };
        let image_pos = args.iter().position(|s| s == &spec.image).unwrap_or(args.len());

        let mut security_flags = Vec::new();
        if profile.read_only_rootfs {
            security_flags.push("--read-only".into());
        }
        if let Some(p) = &profile.seccomp_profile {
            security_flags.push("--security-opt".into());
            security_flags.push(format!("seccomp={}", p));
        }
        for cap in &profile.cap_drop {
            security_flags.push("--cap-drop".into());
            security_flags.push(cap.clone());
        }

        for (i, flag) in security_flags.into_iter().enumerate() {
            args.insert(image_pos + i, flag);
        }
        args
    }

    pub fn wait_args(id: &str) -> Vec<String> {
        vec!["wait".into(), id.into()]
    }

    // ── Output parsers (Docker JSON defaults) ─────────────────────────────

    pub fn parse_list_output(stdout: &str) -> Vec<ContainerInfo> {
        let trimmed = stdout.trim();
        if trimmed.starts_with('[') {
            serde_json::from_str::<Vec<DockerListEntry>>(trimmed)
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.into_container_info())
                .collect()
        } else {
            trimmed
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<DockerListEntry>(l).ok())
                .map(|e| e.into_container_info())
                .collect()
        }
    }

    pub fn parse_inspect_output(id: &str, stdout: &str) -> Option<ContainerInfo> {
        let trimmed = stdout.trim();
        let entry: Option<DockerInspectEntry> = if trimmed.starts_with('[') {
            serde_json::from_str::<Vec<DockerInspectEntry>>(trimmed)
                .ok()
                .and_then(|v| v.into_iter().next())
        } else {
            serde_json::from_str::<DockerInspectEntry>(trimmed).ok()
        };
        entry.map(|e| {
            let running = e.state.as_ref().map(|s| s.running).unwrap_or(false);
            let status = e
                .state
                .as_ref()
                .map(|s| s.status.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| if running { "running" } else { "stopped" }.into());
            ContainerInfo {
                id: if e.id.is_empty() { id.to_string() } else { e.id },
                name: e.name.trim_start_matches('/').to_string(),
                image: e.image,
                status,
                ports: vec![],
                labels: e.config.map(|c| c.labels).unwrap_or_default(),
                created: e.created,
            }
        })
    }

    pub fn parse_list_images_output(stdout: &str) -> Vec<ImageInfo> {
        let trimmed = stdout.trim();
        let entries: Vec<DockerImageEntry> = if trimmed.starts_with('[') {
            serde_json::from_str(trimmed).unwrap_or_default()
        } else {
            trimmed
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        };
        entries
            .into_iter()
            .map(|e| ImageInfo {
                id: e.id,
                repository: e.repository,
                tag: e.tag,
                size: parse_size(&e.size),
                created: e.created,
            })
            .collect()
    }

    pub fn parse_container_id(stdout: &str) -> String {
        stdout.trim().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.6  CliBackend struct
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete `ContainerBackend` that executes CLI commands via
/// `tokio::process::Command`.
pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
    pub driver_name: String,
}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P, driver_name: String) -> Self {
        CliBackend { bin, protocol, driver_name }
    }

    /// Build the full argument list, prepending the protocol's subcommand
    /// prefix (e.g. `["shell", "default", "nerdctl"]` for Lima) when present.
    pub fn full_args(&self, subcommand_args: Vec<String>) -> Vec<String> {
        match self.protocol.subcommand_prefix() {
            Some(prefix) => {
                let mut full = prefix;
                full.extend(subcommand_args);
                full
            }
            None => subcommand_args,
        }
    }

    /// Execute the binary with the given arguments and return the raw output.
    async fn exec_raw(&self, args: Vec<String>) -> Result<std::process::Output> {
        let full = self.full_args(args);
        let output = Command::new(&self.bin)
            .args(&full)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(ComposeError::IoError)?;
        Ok(output)
    }

    /// Execute and return stdout as a `String`, mapping non-zero exit codes to
    /// `ComposeError::BackendError`.
    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let output = self.exec_raw(args).await?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }
}

#[async_trait]
#[async_trait]
impl<P: CliProtocol> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        &self.driver_name
    }

    async fn check_available(&self) -> Result<()> {
        let output = Command::new(&self.bin)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(ComposeError::IoError)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: format!(
                    "'{}' not available: {}",
                    self.backend_name(),
                    String::from_utf8_lossy(&output.stderr)
                ),
            })
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let stdout = self.exec_ok(args).await?;
        let id = OciCommandBuilder::parse_container_id(&stdout);
        let name = spec.name.clone().or_else(|| Some(id.clone()));
        Ok(ContainerHandle { id, name })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let stdout = self.exec_ok(args).await?;
        let id = OciCommandBuilder::parse_container_id(&stdout);
        let name = spec.name.clone().or_else(|| Some(id.clone()));
        Ok(ContainerHandle { id, name })
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
        Ok(OciCommandBuilder::parse_list_output(&stdout))
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self.exec_raw(OciCommandBuilder::inspect_args(id)).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) {
                return Err(ComposeError::NotFound(id.to_string()));
            }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        OciCommandBuilder::parse_inspect_output(id, &stdout)
            .ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let output = self
            .exec_raw(self.protocol.exec_args(id, cmd, env, workdir))
            .await?;
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
        let stdout = self.exec_ok(OciCommandBuilder::list_images_args()).await?;
        Ok(OciCommandBuilder::parse_list_images_output(&stdout))
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.exec_ok(OciCommandBuilder::remove_image_args(reference, force))
            .await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_network_args(name, config))
            .await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let output = self
            .exec_raw(OciCommandBuilder::remove_network_args(name))
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) {
                return Ok(());
            }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_volume_args(name, config))
            .await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let output = self
            .exec_raw(OciCommandBuilder::remove_volume_args(name))
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) {
                return Ok(());
            }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }

    async fn build_image(
        &self,
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        self.exec_ok(self.protocol.build_args(context, tag, dockerfile, args))
            .await?;
        Ok(())
    }

    async fn inspect_image(&self, reference: &str) -> Result<serde_json::Value> {
        let stdout = self
            .exec_ok(OciCommandBuilder::inspect_image_args(reference))
            .await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value> {
        let stdout = self
            .exec_ok(OciCommandBuilder::manifest_inspect_args(reference))
            .await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn inspect_network(&self, name: &str) -> Result<serde_json::Value> {
        let stdout = self
            .exec_ok(OciCommandBuilder::inspect_network_args(name))
            .await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn inspect_volume(&self, name: &str) -> Result<serde_json::Value> {
        let stdout = self
            .exec_ok(OciCommandBuilder::inspect_volume_args(name))
            .await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn run_with_security(
        &self,
        spec: &ContainerSpec,
        profile: &SecurityProfile,
    ) -> Result<ContainerHandle> {
        let mut args = match self.driver_name.as_str() {
            "apple/container" => OciCommandBuilder::apple_run_args(spec),
            _ => OciCommandBuilder::docker_run_args(spec, true),
        };
        let image_pos = args.iter().position(|s| s == &spec.image).unwrap_or(args.len());

        let mut security_flags = Vec::new();
        if profile.read_only_rootfs {
            security_flags.push("--read-only".into());
        }
        if let Some(p) = &profile.seccomp_profile {
            security_flags.push("--security-opt".into());
            security_flags.push(format!("seccomp={}", p));
        }
        for cap in &profile.cap_drop {
            security_flags.push("--cap-drop".into());
            security_flags.push(cap.clone());
        }

        for (i, flag) in security_flags.into_iter().enumerate() {
            args.insert(image_pos + i, flag);
        }

        let stdout = self.exec_ok(args).await?;
        let id = OciCommandBuilder::parse_container_id(&stdout);
        let name = spec.name.clone().or_else(|| Some(id.clone()));
        Ok(ContainerHandle { id, name })
    }

    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs> {
        let _ = self.exec_ok(self.protocol.wait_args(id)).await?;
        self.logs(id, None).await
    }

    fn strategy(&self) -> ExecutionStrategy {
        ExecutionStrategy::CliExec {
            bin: self.bin.clone(),
        }
    }

    fn isolation_level(&self) -> crate::types::IsolationLevel {
        match self.driver_name.as_str() {
            "apple/container" => crate::types::IsolationLevel::Container,
            _ => crate::types::IsolationLevel::Container,
        }
    }
}

const PROBE_TIMEOUT_SECS: u64 = 2;

/// Platform-ordered list of candidate runtime names to probe.
fn platform_candidates() -> &'static [&'static str] {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        &[
            "apple/container",
            "orbstack",
            "colima",
            "rancher-desktop",
            "podman",
            "lima",
            "docker",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        &["podman", "nerdctl", "docker"]
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
    {
        &["podman", "nerdctl", "docker"]
    }
}

/// Run a quick probe command with a timeout and return its stdout.
async fn probe_run(bin: &str, args: &[&str]) -> std::result::Result<String, String> {
    use tokio::time::{timeout, Duration};
    let fut = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match timeout(Duration::from_secs(PROBE_TIMEOUT_SECS), fut).await {
        Ok(Ok(out)) => {
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("probe timed out after {}s", PROBE_TIMEOUT_SECS)),
    }
}

/// Probe a single named runtime and return a type-erased `Box<dyn ContainerBackend>`
/// if it is available, or a human-readable reason string if it is not.
pub async fn probe_candidate(
    name: &str,
) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    match name {
        // ── apple/container ──────────────────────────────────────────────
        "apple/container" => {
            let bin = which::which("container")
                .map_err(|_| "container binary not found on PATH".to_string())?;
            probe_run(bin.to_str().unwrap_or("container"), &["--version"])
                .await
                .map_err(|e| format!("apple/container --version failed: {}", e))?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol, "apple/container".into())))
        }

        // ── orbstack ─────────────────────────────────────────────────────
        "orbstack" => {
            let orb_ok = which::which("orb")
                .ok()
                .map(|b| {
                    let b_str = b.to_string_lossy().to_string();
                    async move { probe_run(&b_str, &["--version"]).await.is_ok() }
                });
            let sock_ok = std::path::Path::new(
                &shellexpand::tilde("~/.orbstack/run/docker.sock").to_string(),
            )
            .exists();
            let orb_available = match orb_ok {
                Some(fut) => fut.await,
                None => false,
            };
            if orb_available || sock_ok {
                let bin = which::which("docker")
                    .or_else(|_| which::which("orb"))
                    .map_err(|_| "orbstack: neither docker nor orb found".to_string())?;
                Ok(Box::new(CliBackend::new(bin, DockerProtocol, "orbstack".into())))
            } else {
                Err("orbstack: neither `orb --version` succeeded nor socket found".into())
            }
        }

        // ── colima ───────────────────────────────────────────────────────
        "colima" => {
            let bin = which::which("colima")
                .map_err(|_| "colima not found".to_string())?;
            let status = probe_run(bin.to_str().unwrap_or("colima"), &["status"])
                .await
                .map_err(|e| format!("colima status failed: {}", e))?;
            if !status.to_lowercase().contains("running") {
                return Err("colima is installed but not running".into());
            }
            let docker_bin = which::which("docker")
                .map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(Box::new(CliBackend::new(docker_bin, DockerProtocol, "colima".into())))
        }

        // ── rancher-desktop ──────────────────────────────────────────────
        "rancher-desktop" => {
            let bin = which::which("nerdctl")
                .map_err(|_| "nerdctl not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("nerdctl"), &["--version"])
                .await
                .map_err(|e| format!("nerdctl --version failed: {}", e))?;
            let sock = std::path::Path::new(
                &shellexpand::tilde("~/.rd/run/containerd-shim.sock").to_string(),
            )
            .exists();
            if sock {
                Ok(Box::new(CliBackend::new(bin, DockerProtocol, "rancher-desktop".into())))
            } else {
                Err("rancher-desktop: nerdctl found but containerd socket missing".into())
            }
        }

        // ── podman ───────────────────────────────────────────────────────
        "podman" => {
            let bin = which::which("podman")
                .map_err(|_| "podman not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("podman"), &["--version"])
                .await
                .map_err(|e| format!("podman --version failed: {}", e))?;

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                let machines = probe_run(
                    bin.to_str().unwrap_or("podman"),
                    &["machine", "list", "--format", "json"],
                )
                .await
                .unwrap_or_default();
                let has_running = serde_json::from_str::<Vec<serde_json::Value>>(&machines)
                    .unwrap_or_default()
                    .iter()
                    .any(|m| m.get("Running").and_then(|v| v.as_bool()).unwrap_or(false));
                if !has_running {
                    return Err(
                        "podman: no running machine found (run `podman machine start`)".into(),
                    );
                }
            }

            Ok(Box::new(CliBackend::new(bin, DockerProtocol, "podman".into())))
        }

        // ── lima ─────────────────────────────────────────────────────────
        "lima" => {
            let bin = which::which("limactl")
                .map_err(|_| "limactl not found".to_string())?;
            let list_out = probe_run(bin.to_str().unwrap_or("limactl"), &["list", "--json"])
                .await
                .map_err(|e| format!("limactl list --json failed: {}", e))?;
            let instance = list_out
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| {
                    v.get("status")
                        .and_then(|s| s.as_str())
                        .map(|s| s.eq_ignore_ascii_case("running"))
                        .unwrap_or(false)
                })
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .ok_or_else(|| "limactl: no running Lima instance found".to_string())?;
            Ok(Box::new(CliBackend::new(bin, LimaProtocol { instance }, "lima".into())))
        }

        // ── nerdctl (standalone) ─────────────────────────────────────────
        "nerdctl" => {
            let bin = which::which("nerdctl")
                .map_err(|_| "nerdctl not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("nerdctl"), &["--version"])
                .await
                .map_err(|e| format!("nerdctl --version failed: {}", e))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol, "nerdctl".into())))
        }

        // ── docker ───────────────────────────────────────────────────────
        "docker" => {
            let bin = which::which("docker")
                .map_err(|_| "docker not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("docker"), &["--version"])
                .await
                .map_err(|e| format!("docker --version failed: {}", e))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol, "docker".into())))
        }

        other => Err(format!("unknown runtime '{}'", other)),
    }
}

/// Detect the best available container backend for the current platform.
///
/// 1. If `PERRY_CONTAINER_BACKEND` is set, use that backend directly.
/// 2. Otherwise, probe `platform_candidates()` in order with a 2s timeout each.
/// 3. If no candidate is available, returns `Err(NoBackendFound { probed })`.
pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, ComposeError> {
    use std::time::Duration;

    let mode = std::env::var("PERRY_CONTAINER_MODE").unwrap_or_else(|_| "local-first".into());

    // ── Override via env var ──────────────────────────────────────────────
    if let Ok(override_name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let name = override_name.trim().to_string();
        debug!("PERRY_CONTAINER_BACKEND={}, probing directly", name);
        return probe_candidate(&name).await.map_err(|reason| {
            ComposeError::BackendNotAvailable {
                name: name.clone(),
                reason,
            }
        });
    }

    if mode == "server-first" {
        // In server-first mode, we'd prefer remote daemon connections.
        // For now, this is a placeholder for that logic.
    }

    // ── Platform probe sequence ───────────────────────────────────────────
    let mut probed: Vec<BackendProbeResult> = Vec::new();

    for &candidate in platform_candidates() {
        debug!("probing container backend: {}", candidate);
        match tokio::time::timeout(
            Duration::from_secs(PROBE_TIMEOUT_SECS),
            probe_candidate(candidate),
        )
        .await
        {
            Ok(Ok(backend)) => {
                debug!("selected container backend: {}", candidate);
                return Ok(backend);
            }
            Ok(Err(reason)) => {
                debug!("backend '{}' not available: {}", candidate, reason);
                probed.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: false,
                    reason,
                });
            }
            Err(_) => {
                debug!("backend '{}' probe timed out", candidate);
                probed.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: false,
                    reason: format!("probe timed out after {}s", PROBE_TIMEOUT_SECS),
                });
            }
        }
    }

    Err(ComposeError::NoBackendFound { probed })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_spec(name: Option<&str>) -> ContainerSpec {
        ContainerSpec {
            image: "alpine:latest".into(),
            name: name.map(String::from),
            ports: Some(vec!["8080:80".into()]),
            volumes: Some(vec!["/tmp:/data".into()]),
            env: Some({
                let mut m = HashMap::new();
                m.insert("FOO".into(), "bar".into());
                m
            }),
            cmd: Some(vec!["sh".into(), "-c".into(), "echo hi".into()]),
            entrypoint: None,
            network: Some("mynet".into()),
            rm: Some(true),
        }
    }

    // ── OciCommandBuilder ─────────────────────────────────────────────────

    #[test]
    fn docker_run_args_contains_expected_flags() {
        let _drv = BackendDriver::Docker { bin: "docker".into() };
        let spec = dummy_spec(Some("mycontainer"));
        let args = OciCommandBuilder::docker_run_args(&spec, true);
        assert!(args.contains(&"run".into()));
        assert!(args.contains(&"--rm".into()));
        assert!(args.contains(&"--detach".into()));
        assert!(args.contains(&"--name".into()));
        assert!(args.contains(&"mycontainer".into()));
        assert!(args.contains(&"-p".into()));
        assert!(args.contains(&"8080:80".into()));
        assert!(args.contains(&"-v".into()));
        assert!(args.contains(&"/tmp:/data".into()));
        assert!(args.contains(&"-e".into()));
        assert!(args.contains(&"FOO=bar".into()));
        assert!(args.contains(&"--network".into()));
        assert!(args.contains(&"mynet".into()));
        assert!(args.contains(&"alpine:latest".into()));
    }

    #[test]
    fn docker_stop_args_with_timeout() {
        let _drv = BackendDriver::Docker { bin: "docker".into() };
        let args = OciCommandBuilder::stop_args( "abc123", Some(10));
        assert_eq!(args, vec!["stop", "-t", "10", "abc123"]);
    }

    #[test]
    fn docker_stop_args_no_timeout() {
        let _drv = BackendDriver::Docker { bin: "docker".into() };
        let args = OciCommandBuilder::stop_args( "abc123", None);
        assert_eq!(args, vec!["stop", "abc123"]);
    }

    #[test]
    fn docker_remove_args_force() {
        let _drv = BackendDriver::Docker { bin: "docker".into() };
        assert_eq!(OciCommandBuilder::remove_args( "c1", true), vec!["rm", "-f", "c1"]);
        assert_eq!(OciCommandBuilder::remove_args( "c1", false), vec!["rm", "c1"]);
    }

    #[test]
    fn docker_list_args() {
        let _drv = BackendDriver::Docker { bin: "docker".into() };
        assert!(OciCommandBuilder::list_args( true).contains(&"--all".into()));
        assert!(!OciCommandBuilder::list_args( false).contains(&"--all".into()));
    }

    #[test]
    fn docker_parse_list_output_array() {
        let json = r#"[{"ID":"abc","Names":["/myapp"],"Image":"nginx","Status":"running","Ports":["80/tcp"],"Created":"2024-01-01"}]"#;
        let infos = OciCommandBuilder::parse_list_output(json);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "abc");
        assert_eq!(infos[0].name, "myapp");
    }

    #[test]
    fn docker_parse_list_output_ndjson() {
        let json = "{\"ID\":\"abc\",\"Names\":[\"/myapp\"],\"Image\":\"nginx\",\"Status\":\"running\",\"Ports\":[],\"Created\":\"2024-01-01\"}\n{\"ID\":\"def\",\"Names\":[\"/other\"],\"Image\":\"redis\",\"Status\":\"stopped\",\"Ports\":[],\"Created\":\"2024-01-02\"}";
        let infos = OciCommandBuilder::parse_list_output(json);
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn docker_parse_inspect_output() {
        let json = r#"[{"Id":"abc123","Name":"/myapp","Image":"nginx","State":{"Running":true,"Status":"running"},"Created":"2024-01-01"}]"#;
        let info = OciCommandBuilder::parse_inspect_output("abc123", json).unwrap();
        assert_eq!(info.status, "running");
        assert_eq!(info.name, "myapp");
    }

    #[test]
    fn docker_parse_images_output() {
        let json = r#"[{"ID":"sha256:abc","Repository":"nginx","Tag":"latest","Size":50000000,"Created":"2024-01-01"}]"#;
        let images = OciCommandBuilder::parse_list_images_output(json);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].repository, "nginx");
        assert_eq!(images[0].size, 50_000_000);
    }

    // ── NetworkConfig / VolumeConfig args ─────────────────────────────────

    #[test]
    fn create_network_args_with_config() {
        let _drv = BackendDriver::Docker { bin: "docker".into() };
        let mut labels = HashMap::new();
        labels.insert("env".into(), "prod".into());
        let config = NetworkConfig {
            driver: Some("bridge".into()),
            labels,
            internal: true,
            enable_ipv6: false,
        };
        let args = OciCommandBuilder::create_network_args( "mynet", &config);
        assert!(args.contains(&"network".into()));
        assert!(args.contains(&"create".into()));
        assert!(args.contains(&"--driver".into()));
        assert!(args.contains(&"bridge".into()));
        assert!(args.contains(&"--label".into()));
        assert!(args.contains(&"env=prod".into()));
        assert!(args.contains(&"--internal".into()));
        assert!(!args.contains(&"--ipv6".into()));
        assert!(args.last() == Some(&"mynet".into()));
    }

    #[test]
    fn create_volume_args_with_config() {
        let _drv = BackendDriver::Docker { bin: "docker".into() };
        let config = VolumeConfig {
            driver: Some("local".into()),
            labels: HashMap::new(),
        };
        let args = OciCommandBuilder::create_volume_args( "myvol", &config);
        assert!(args.contains(&"volume".into()));
        assert!(args.contains(&"create".into()));
        assert!(args.contains(&"--driver".into()));
        assert!(args.contains(&"local".into()));
        assert!(args.last() == Some(&"myvol".into()));
    }

    // ── From conversions ──────────────────────────────────────────────────

    #[test]
    fn network_config_from_compose_network() {
        use crate::types::ListOrDict;
        let mut cn = ComposeNetwork::default();
        cn.driver = Some("overlay".into());
        cn.internal = Some(true);
        cn.enable_ipv6 = Some(true);
        cn.labels = Some(ListOrDict::List(vec!["foo=bar".into()]));
        let nc = NetworkConfig::from(&cn);
        assert_eq!(nc.driver, Some("overlay".into()));
        assert!(nc.internal);
        assert!(nc.enable_ipv6);
        assert_eq!(nc.labels.get("foo"), Some(&"bar".into()));
    }

    #[test]
    fn volume_config_from_compose_volume() {
        use crate::types::ListOrDict;
        let mut cv = ComposeVolume::default();
        cv.driver = Some("nfs".into());
        cv.labels = Some(ListOrDict::List(vec!["tier=data".into()]));
        let vc = VolumeConfig::from(&cv);
        assert_eq!(vc.driver, Some("nfs".into()));
        assert_eq!(vc.labels.get("tier"), Some(&"data".into()));
    }


    // ── BackendProbeResult serialization ─────────────────────────────────

    #[test]
    fn probe_result_round_trip() {
        let r = BackendProbeResult {
            name: "podman".into(),
            available: false,
            reason: "not found".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: BackendProbeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r2.name, "podman");
        assert!(!r2.available);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // Feature: alloy-container, Property 3: ContainerSpec CLI arg round-trip
    proptest! {
        #[test]
        fn test_docker_run_flags_completeness(
            image in ".*",
            name in prop::option::of(".*"),
            network in prop::option::of(".*"),
            rm in prop::option::of(any::<bool>())
        ) {
            let spec = ContainerSpec {
                image,
                name: name.clone(),
                network: network.clone(),
                rm,
                ..Default::default()
            };
            let flags = docker_run_flags(&spec, true);
            if let Some(n) = name {
                assert!(flags.contains(&"--name".into()));
                assert!(flags.contains(&n));
            }
            if let Some(net) = network {
                assert!(flags.contains(&"--network".into()));
                assert!(flags.contains(&net));
            }
            if rm.unwrap_or(false) {
                assert!(flags.contains(&"--rm".into()));
            }
            assert!(flags.contains(&"--detach".into()));
        }
    }
}
