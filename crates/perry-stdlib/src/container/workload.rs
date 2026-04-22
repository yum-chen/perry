//! Workload graph types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::container::types::ContainerInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PolicyTier {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: PolicyTier,
    #[serde(default)]
    pub no_network: bool,
    #[serde(default)]
    pub read_only_root: bool,
    #[serde(default)]
    pub seccomp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, String> {
        let info = running_nodes.get(&self.node_id).ok_or_else(|| format!("Node {} not found", self.node_id))?;
        match self.projection {
            RefProjection::Endpoint => {
                let port = self.port.as_deref().unwrap_or("80");
                // In a real implementation we'd find the mapped port
                Ok(format!("{}:{}", info.id, port))
            }
            RefProjection::Ip => Ok(info.id.clone()),
            RefProjection::InternalUrl => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("http://{}:{}", info.id, port))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: ExecutionStrategy,
    pub on_failure: FailureStrategy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workload_ref_resolution() {
        let mut nodes = HashMap::new();
        nodes.insert("db".to_string(), ContainerInfo {
            id: "container-db-123".to_string(),
            name: "db".to_string(),
            image: "postgres".to_string(),
            status: "running".to_string(),
            ports: vec!["5432:5432".to_string()],
            created: "".to_string(),
        });

        let r = WorkloadRef {
            node_id: "db".to_string(),
            projection: RefProjection::Endpoint,
            port: Some("5432".to_string()),
        };
        assert_eq!(r.resolve(&nodes).unwrap(), "container-db-123:5432");

        let r2 = WorkloadRef {
            node_id: "db".to_string(),
            projection: RefProjection::Ip,
            port: None,
        };
        assert_eq!(r2.resolve(&nodes).unwrap(), "container-db-123");
    }
}
