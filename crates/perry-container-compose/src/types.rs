//! All compose-spec Rust types.
//!
//! This module contains every struct and enum needed to represent a
//! compose-spec YAML document, plus the opaque `ComposeHandle` returned by
//! `ComposeEngine::up()`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Convert a `serde_yaml::Value` to a string representation.
fn yaml_value_to_str(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        _ => serde_yaml::to_string(v).unwrap_or_default().trim().to_owned(),
    }
}

// ============ ListOrDict ============

/// The compose-spec list_or_dict pattern.
/// Used for environment, labels, extra_hosts, sysctls, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(IndexMap<String, Option<serde_yaml::Value>>),
    List(Vec<String>),
}

impl ListOrDict {
    /// Convert to a flat `HashMap<String, String>`.
    /// Dict values are stringified; List entries are split on `=`.
    pub fn to_map(&self) -> std::collections::HashMap<String, String> {
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

// ============ DependsOn ============

/// depends_on condition values (compose-spec §service.depends_on)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

/// Per-dependency entry in the object form of depends_on
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDependsOn {
    pub condition: DependsOnCondition,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub restart: Option<bool>,
}

/// depends_on can be a list of service names or a map with conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ============ Volume ============

/// Volume mount type (compose-spec §service.volumes[].type)
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

/// Long-form volume mount (compose-spec §service.volumes[])
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeBind {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
    pub recursive: Option<String>,
    pub selinux: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeOpts {
    pub labels: Option<ListOrDict>,
    pub nocopy: Option<bool>,
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeTmpfs {
    pub size: Option<serde_yaml::Value>,  // string or number
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeImage {
    pub subpath: Option<String>,
}

/// Short or long volume form
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VolumeSpec {
    Short(String),
    Long(ComposeServiceVolume),
}

impl VolumeSpec {
    /// Convert to "source:target[:ro]" string form for backend CLI args.
    pub fn to_string_form(&self) -> String {
        match self {
            VolumeSpec::Short(s) => s.clone(),
            VolumeSpec::Long(v) => {
                let src = v.source.as_deref().unwrap_or("");
                let tgt = v.target.as_deref().unwrap_or("");
                if v.read_only.unwrap_or(false) {
                    format!("{}:{}:ro", src, tgt)
                } else {
                    format!("{}:{}", src, tgt)
                }
            }
        }
    }
}

// ============ Port ============

/// Port mapping (long form, compose-spec §service.ports[])
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub host_ip: Option<String>,
    pub target: serde_yaml::Value,      // integer or string
    pub published: Option<serde_yaml::Value>, // string or integer
    pub protocol: Option<String>,
    pub app_protocol: Option<String>,
}

/// Port can be a short string/number or a long-form object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PortSpec {
    Short(serde_yaml::Value),  // "8080:80" or 8080
    Long(ComposeServicePort),
}

impl PortSpec {
    /// Convert to "host:container" string form for backend CLI args.
    pub fn to_string_form(&self) -> String {
        match self {
            PortSpec::Short(v) => yaml_value_to_str(v),
            PortSpec::Long(p) => {
                let container = yaml_value_to_str(&p.target);
                match &p.published {
                    Some(pub_) => {
                        let host = yaml_value_to_str(pub_);
                        format!("{}:{}", host, container)
                    }
                    None => container,
                }
            }
        }
    }
}

// ============ Networks on service ============

/// Service network attachment config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub priority: Option<i32>,
}

/// networks field on a service: list or map
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ============ Build ============

/// Build configuration (string shorthand or full object)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildSpec {
    Context(String),
    Config(ComposeServiceBuild),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

// ============ Healthcheck ============

/// Healthcheck configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHealthcheck {
    pub test: serde_yaml::Value,  // string or string[]
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

// ============ Deployment ============

/// Deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeploymentResources {
    pub limits: Option<ComposeResourceSpec>,
    pub reservations: Option<ComposeResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeResourceSpec {
    pub cpus: Option<serde_yaml::Value>,
    pub memory: Option<String>,
    pub pids: Option<i64>,
}

// ============ Logging ============

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<IndexMap<String, serde_yaml::Value>>,
}

// ============ Network ============

/// IPAM configuration for a network
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<ComposeNetworkIpamConfig>>,
    pub options: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpamConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<IndexMap<String, String>>,
}

/// Top-level network definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

// ============ Volume ============

/// Top-level volume definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

// ============ Secret ============

/// Top-level secret definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

// ============ Config ============

/// Top-level config definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeConfig {
    pub name: Option<String>,
    pub content: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    pub template_driver: Option<String>,
}

// ============ ComposeService ============

/// Full service definition (compose-spec §service)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<BuildSpec>,
    pub command: Option<serde_yaml::Value>,      // string or string[]
    pub entrypoint: Option<serde_yaml::Value>,   // string or string[]
    pub environment: Option<ListOrDict>,
    pub env_file: Option<serde_yaml::Value>,     // string or string[]
    pub ports: Option<Vec<PortSpec>>,
    pub volumes: Option<Vec<serde_yaml::Value>>, // string or ComposeServiceVolume
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
    #[serde(rename = "isolation_level")]
    pub isolation_level: Option<IsolationLevel>,
}

impl ComposeService {
    /// Get resolved environment as a flat map.
    pub fn resolved_env(&self) -> std::collections::HashMap<String, String> {
        self.environment
            .as_ref()
            .map(|e| e.to_map())
            .unwrap_or_default()
    }

    /// Get port strings in "host:container" form.
    pub fn port_strings(&self) -> Vec<String> {
        self.ports
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|p| p.to_string_form())
            .collect()
    }

    /// Get volume mount strings.
    pub fn volume_strings(&self) -> Vec<String> {
        self.volumes
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|v| {
                if let Ok(spec) = serde_yaml::from_value::<VolumeSpec>(v.clone()) {
                    spec.to_string_form()
                } else {
                    yaml_value_to_str(v)
                }
            })
            .collect()
    }

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

// ============ ComposeSpec ============

/// Root compose spec (compose-spec §root)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

// ============ ComposeHandle ============

/// Opaque handle to a running compose stack.
/// The stack ID is used to look up the live ComposeEngine in a global registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

// ============ Isolation and Backend Info ============

/// Isolation level of the container runtime.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IsolationLevel {
    None,
    Process,
    Container,
    MicroVm,
    Wasm,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::Container
    }
}

/// Information about a detected container backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendInfo {
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
    pub version: Option<String>,
    pub mode: String, // "local" | "remote"
    pub isolation_level: IsolationLevel,
}

// ============ Container types (for single-container API) ============

/// Specification for running a single container.
/// Canonical fields from SPEC.md §2.3.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
    pub read_only: Option<bool>,
    pub security_opt: Option<Vec<String>>,
    pub isolation_level: Option<IsolationLevel>,
}

/// Handle returned after creating/running a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

/// Information about a running (or stopped) container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
    pub ip: Option<String>,
}

/// Logs from a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

/// Information about a container image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}
