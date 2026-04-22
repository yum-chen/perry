//! All compose-spec and container Rust types.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Convert a `serde_yaml::Value` to a string representation.
fn yaml_value_to_str(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        _ => format!("{}", serde_yaml::to_string(v).unwrap_or_default()).trim().to_owned(),
    }
}

// ============ ListOrDict ============

/// compose-spec `list_or_dict` pattern.
/// Used for environment, labels, extra_hosts, sysctls, etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(IndexMap<String, Option<serde_yaml::Value>>),
    List(Vec<String>),
}

impl ListOrDict {
    /// Convert to a flat `HashMap<String, String>`.
    pub fn to_map(&self) -> HashMap<String, String> {
        match self {
            ListOrDict::Dict(map) => map
                .iter()
                .map(|(k, v)| {
                    let val = match v {
                        Some(serde_yaml::Value::String(s)) => s.clone(),
                        Some(serde_yaml::Value::Number(n)) => n.to_string(),
                        Some(serde_yaml::Value::Bool(b)) => b.to_string(),
                        Some(serde_yaml::Value::Null) | None => String::new(),
                        Some(other) => yaml_value_to_str(other),
                    };
                    (k.clone(), val)
                })
                .collect(),
            ListOrDict::List(list) => list
                .iter()
                .filter_map(|entry| {
                    let mut parts = entry.splitn(2, '=');
                    let key = parts.next()?.to_owned();
                    let val = parts.next().unwrap_or("").to_owned();
                    Some((key, val))
                })
                .collect(),
        }
    }
}

// ============ Container types (SPEC.md §2.3) ============

/// Specification for running a single container.
/// Requirement 2.7, Task 2.1: Exactly 9 canonical fields.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
}

/// Handle returned after creating/running a container.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

/// Information about a running (or stopped) container.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
}

/// Logs from a container.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

impl IntoIterator for ContainerLogs {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        vec![("stdout".to_string(), self.stdout), ("stderr".to_string(), self.stderr)].into_iter()
    }
}

/// Information about a container image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IsolationLevel {
    None,
    Process,
    Container,
    MicroVm,
    Wasm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendInfo {
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
    pub version: Option<String>,
    pub mode: String, // "local" | "remote"
    #[serde(rename = "isolationLevel")]
    pub isolation_level: IsolationLevel,
}

// ============ Compose Types ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeDependsOn {
    pub condition: DependsOnCondition,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub restart: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DependsOnSpec {
    List(Vec<String>),
    Map(IndexMap<String, ComposeDependsOn>),
}

impl DependsOnSpec {
    pub fn service_names(&self) -> Vec<String> {
        match self {
            DependsOnSpec::List(names) => names.clone(),
            DependsOnSpec::Map(map) => map.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VolumeType {
    Bind,
    Volume,
    Tmpfs,
    Cluster,
    Npipe,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeServiceVolume {
    #[serde(rename = "type")]
    pub volume_type: VolumeType,
    pub source: Option<String>,
    pub target: Option<String>,
    pub read_only: Option<bool>,
    pub consistency: Option<String>,
    pub bind: Option<ComposeServiceVolumeBind>,
    pub volume: Option<ComposeServiceVolumeOpts>,
    pub tmpfs: Option<ComposeServiceVolumeTmpfs>,
    pub image: Option<ComposeServiceVolumeImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeServiceVolumeBind {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
    pub recursive: Option<String>,
    pub selinux: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeServiceVolumeOpts {
    pub labels: Option<ListOrDict>,
    pub nocopy: Option<bool>,
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeServiceVolumeTmpfs {
    pub size: Option<serde_yaml::Value>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeServiceVolumeImage {
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub host_ip: Option<String>,
    pub target: serde_yaml::Value,
    pub published: Option<serde_yaml::Value>,
    pub protocol: Option<String>,
    pub app_protocol: Option<String>,
}

impl ComposeServicePort {
    pub fn to_string_form(&self) -> String {
        let container = yaml_value_to_str(&self.target);
        match &self.published {
            Some(pub_) => {
                let host = yaml_value_to_str(pub_);
                format!("{}:{}", host, container)
            }
            None => container,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PortSpec {
    Short(serde_yaml::Value),
    Long(ComposeServicePort),
}

impl PortSpec {
    pub fn to_string_form(&self) -> String {
        match self {
            PortSpec::Short(v) => yaml_value_to_str(v),
            PortSpec::Long(p) => p.to_string_form(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ServiceNetworks {
    List(Vec<String>),
    Map(IndexMap<String, Option<ComposeServiceNetworkConfig>>),
}

impl ServiceNetworks {
    pub fn names(&self) -> Vec<String> {
        match self {
            ServiceNetworks::List(v) => v.clone(),
            ServiceNetworks::Map(m) => m.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeServiceBuild {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub dockerfile_inline: Option<String>,
    pub args: Option<ListOrDict>,
    pub ssh: Option<serde_yaml::Value>,
    pub labels: Option<ListOrDict>,
    pub cache_from: Option<Vec<String>>,
    pub cache_to: Option<Vec<String>>,
    pub no_cache: Option<bool>,
    pub additional_contexts: Option<IndexMap<String, String>>,
    pub network: Option<String>,
    pub provenance: Option<serde_yaml::Value>,
    pub sbom: Option<serde_yaml::Value>,
    pub pull: Option<bool>,
    pub target: Option<String>,
    pub shm_size: Option<serde_yaml::Value>,
    pub extra_hosts: Option<ListOrDict>,
    pub isolation: Option<String>,
    pub privileged: Option<bool>,
    pub secrets: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub ulimits: Option<serde_yaml::Value>,
    pub platforms: Option<Vec<String>>,
    pub entitlements: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum BuildSpec {
    Context(String),
    Config(ComposeServiceBuild),
}

impl BuildSpec {
    pub fn as_build(&self) -> ComposeServiceBuild {
        match self {
            BuildSpec::Context(ctx) => ComposeServiceBuild {
                context: Some(ctx.clone()),
                ..Default::default()
            },
            BuildSpec::Config(b) => b.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeHealthcheck {
    pub test: serde_yaml::Value,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeDeployment {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<ListOrDict>,
    pub resources: Option<ComposeDeploymentResources>,
    pub restart_policy: Option<serde_yaml::Value>,
    pub placement: Option<serde_yaml::Value>,
    pub update_config: Option<serde_yaml::Value>,
    pub rollback_config: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeDeploymentResources {
    pub limits: Option<ComposeResourceSpec>,
    pub reservations: Option<ComposeResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeResourceSpec {
    pub cpus: Option<serde_yaml::Value>,
    pub memory: Option<String>,
    pub pids: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<IndexMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeNetworkIpamConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<ComposeNetworkIpamConfig>>,
    pub options: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, String>>,
    pub ipam: Option<ComposeNetworkIpam>,
    pub external: Option<bool>,
    pub internal: Option<bool>,
    pub enable_ipv4: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub attachable: Option<bool>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeSecret {
    pub name: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, String>>,
    pub template_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeConfig {
    pub name: Option<String>,
    pub content: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    pub template_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<BuildSpec>,
    pub command: Option<serde_yaml::Value>,
    pub entrypoint: Option<serde_yaml::Value>,
    pub environment: Option<ListOrDict>,
    pub env_file: Option<serde_yaml::Value>,
    pub ports: Option<Vec<PortSpec>>,
    pub volumes: Option<Vec<serde_yaml::Value>>,
    pub networks: Option<ServiceNetworks>,
    pub depends_on: Option<DependsOnSpec>,
    pub restart: Option<String>,
    pub healthcheck: Option<ComposeHealthcheck>,
    pub container_name: Option<String>,
    pub labels: Option<ListOrDict>,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub working_dir: Option<String>,
    pub privileged: Option<bool>,
    pub read_only: Option<bool>,
    pub stdin_open: Option<bool>,
    pub tty: Option<bool>,
    pub stop_signal: Option<String>,
    pub stop_grace_period: Option<String>,
    pub network_mode: Option<String>,
    pub pid: Option<String>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub security_opt: Option<Vec<String>>,
    pub sysctls: Option<ListOrDict>,
    pub ulimits: Option<serde_yaml::Value>,
    pub logging: Option<ComposeLogging>,
    pub deploy: Option<ComposeDeployment>,
    pub develop: Option<serde_yaml::Value>,
    pub secrets: Option<Vec<String>>,
    pub configs: Option<Vec<String>>,
    pub expose: Option<Vec<serde_yaml::Value>>,
    pub extra_hosts: Option<ListOrDict>,
    pub dns: Option<serde_yaml::Value>,
    pub dns_search: Option<serde_yaml::Value>,
    pub tmpfs: Option<serde_yaml::Value>,
    pub shm_size: Option<serde_yaml::Value>,
    pub mem_limit: Option<serde_yaml::Value>,
    pub memswap_limit: Option<serde_yaml::Value>,
    pub cpus: Option<serde_yaml::Value>,
    pub cpu_shares: Option<i64>,
    pub platform: Option<String>,
    pub pull_policy: Option<String>,
    pub profiles: Option<Vec<String>>,
    pub scale: Option<u32>,
    pub extends: Option<serde_yaml::Value>,
    pub post_start: Option<Vec<serde_yaml::Value>>,
    pub pre_stop: Option<Vec<serde_yaml::Value>>,
    pub isolation_level: Option<IsolationLevel>,
}

impl ComposeService {
    /// Get command as a list of strings.
    pub fn command_list(&self) -> Option<Vec<String>> {
        self.command.as_ref().map(|c| match c {
            serde_yaml::Value::String(s) => vec![s.clone()],
            serde_yaml::Value::Sequence(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => vec![],
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComposeSpec {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub services: IndexMap<String, ComposeService>,
    pub networks: Option<IndexMap<String, Option<ComposeNetwork>>>,
    pub volumes: Option<IndexMap<String, Option<ComposeVolume>>>,
    pub secrets: Option<IndexMap<String, Option<ComposeSecret>>>,
    pub configs: Option<IndexMap<String, Option<ComposeConfig>>>,
    pub include: Option<Vec<serde_yaml::Value>>,
    pub models: Option<IndexMap<String, serde_yaml::Value>>,
    #[serde(flatten)]
    pub extensions: IndexMap<String, serde_yaml::Value>,
}

impl ComposeSpec {
    /// Parse from a YAML string.
    pub fn parse_str(yaml: &str) -> Result<Self, crate::error::ComposeError> {
        serde_yaml::from_str(yaml).map_err(crate::error::ComposeError::ParseError)
    }

    /// Merge another ComposeSpec into this one (last-writer-wins for all maps).
    pub fn merge(&mut self, other: ComposeSpec) {
        for (name, service) in other.services {
            self.services.insert(name, service);
        }

        if let Some(nets) = other.networks {
            let existing = self.networks.get_or_insert_with(IndexMap::new);
            for (name, net) in nets {
                existing.insert(name, net);
            }
        }

        if let Some(vols) = other.volumes {
            let existing = self.volumes.get_or_insert_with(IndexMap::new);
            for (name, vol) in vols {
                existing.insert(name, vol);
            }
        }

        if let Some(secs) = other.secrets {
            let existing = self.secrets.get_or_insert_with(IndexMap::new);
            for (name, sec) in secs {
                existing.insert(name, sec);
            }
        }

        if let Some(cfgs) = other.configs {
            let existing = self.configs.get_or_insert_with(IndexMap::new);
            for (name, cfg) in cfgs {
                existing.insert(name, cfg);
            }
        }

        if other.name.is_some() {
            self.name = other.name;
        }
        if other.version.is_some() {
            self.version = other.version;
        }

        // Merge extensions
        for (k, v) in other.extensions {
            self.extensions.insert(k, v);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceGraphEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<ServiceGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceStatus {
    pub service: String,
    pub state: String, // "running" | "stopped" | "failed" | "pending" | "unknown"
    #[serde(rename = "containerId")]
    pub container_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StackStatus {
    pub services: Vec<ServiceStatus>,
    pub healthy: bool,
}

// ============ Workload Graph Types (Task 14) ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: PolicyTier,
    pub no_network: Option<bool>,
    pub read_only_root: Option<bool>,
    pub seccomp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyTier {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub resources: Option<serde_json::Value>,
    pub ports: Vec<String>,
    pub env: HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
    pub runtime: RuntimeSpec,
    pub policy: PolicySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<String>, // "started" | "healthy" | "completed"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: Option<ExecutionStrategy>,
    pub on_failure: Option<FailureStrategy>,
}

impl Default for RunGraphOptions {
    fn default() -> Self {
        Self {
            strategy: Some(ExecutionStrategy::DependencyAware),
            on_failure: Some(FailureStrategy::RollbackAll),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphHandle {
    pub id: u64,
}
