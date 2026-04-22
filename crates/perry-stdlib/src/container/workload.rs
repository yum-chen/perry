use serde::{Deserialize, Serialize};
use indexmap::IndexMap;
use std::collections::HashMap;
use crate::container::types::{ContainerInfo, ContainerError, IsolationLevel};

/// Explicit runtime selection for a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeSpec {
    Oci,
    MicroVm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

impl Default for RuntimeSpec {
    fn default() -> Self {
        Self::Auto
    }
}

/// Per-node isolation policy.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PolicySpec {
    pub tier: PolicyTier,
    #[serde(default)]
    pub no_network: bool,
    #[serde(default)]
    pub read_only_root: bool,
    #[serde(default)]
    pub seccomp: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyTier {
    #[default]
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

/// Typed cross-node reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, ContainerError> {
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| ContainerError::NotFound(format!("Node {} not found for reference", self.node_id)))?;

        match self.projection {
            RefProjection::Ip => Ok(info.id.clone()), // Use ID as placeholder for IP in CLI-based backends
            RefProjection::Endpoint => {
                let port = self.port.as_ref().ok_or_else(|| ContainerError::InvalidConfig("Port required for endpoint projection".into()))?;
                Ok(format!("{}:{}", info.id, port))
            }
            RefProjection::InternalUrl => {
                Ok(format!("http://{}", info.id))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub resources: Option<WorkloadResources>,
    pub ports: Vec<String>,
    pub env: HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub runtime: RuntimeSpec,
    #[serde(default)]
    pub policy: PolicySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadResources {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGraphOptions {
    pub strategy: Option<ExecutionStrategy>,
    pub on_failure: Option<FailureStrategy>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    #[default]
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FailureStrategy {
    #[default]
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}
