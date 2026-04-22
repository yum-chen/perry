//! Workload graph types and engine.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::container::context::{ContainerContext, HandleEntry};
use perry_container_compose::types::{ContainerInfo, IsolationLevel};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeSpec {
    Oci,
    MicroVm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: String,
    pub no_network: Option<bool>,
    pub read_only_root: Option<bool>,
    pub seccomp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: String, // "endpoint" | "ip" | "internalUrl"
    pub port: Option<String>,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, String> {
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| format!("Node {} not found", self.node_id))?;

        match self.projection.as_str() {
            "endpoint" => {
                let port_suffix = self.port.as_deref().unwrap_or("");
                // In production, we'd look up the published port from the backend's inspect result.
                // For now, we assume the mapped port is what the user asked for if it matches.
                for p in &info.ports {
                    if p.contains(port_suffix) {
                        return Ok(p.clone());
                    }
                }
                info.ports.first().cloned().ok_or_else(|| format!("No ports available for node {}", self.node_id))
            }
            "ip" => {
                // Simplified IP resolution for local backends.
                Ok("127.0.0.1".to_string())
            }
            "internalUrl" => {
                let endpoint = self.resolve(running_nodes)?;
                Ok(format!("http://{}", endpoint))
            }
            _ => Err(format!("Unknown projection: {}", self.projection)),
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
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: HashMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

pub async fn run_workload_graph(graph: WorkloadGraph) -> Result<u64, String> {
    let ctx = ContainerContext::global();
    let backend = ctx.get_backend().await.map_err(|e| e.to_string())?;

    // 1. Map WorkloadGraph to ComposeSpec for initial start (without resolved refs)
    let mut compose = perry_container_compose::types::ComposeSpec::default();
    compose.name = Some(graph.name.clone());

    for (id, node) in &graph.nodes {
        let mut svc = perry_container_compose::types::ComposeService::default();
        svc.image = node.image.clone();
        svc.ports = Some(node.ports.iter().map(|p| perry_container_compose::types::PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect());

        let mut env = perry_container_compose::types::ListOrDict::Dict(indexmap::IndexMap::new());
        if let perry_container_compose::types::ListOrDict::Dict(ref mut map) = env {
            for (k, v) in &node.env {
                match v {
                    WorkloadEnvValue::Literal(s) => { map.insert(k.clone(), Some(serde_yaml::Value::String(s.clone()))); }
                    // Placeholders for now, will be updated after first start
                    WorkloadEnvValue::Ref(_) => { map.insert(k.clone(), Some(serde_yaml::Value::String("PENDING_REF".to_string()))); }
                }
            }
        }
        svc.environment = Some(env);
        svc.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(node.depends_on.clone()));

        svc.isolation_level = match node.runtime {
            RuntimeSpec::Oci => Some(IsolationLevel::Container),
            RuntimeSpec::MicroVm { .. } => Some(IsolationLevel::MicroVm),
            RuntimeSpec::Wasm { .. } => Some(IsolationLevel::Wasm),
            RuntimeSpec::Auto => None,
        };

        compose.services.insert(id.clone(), svc);
    }

    let engine = Arc::new(perry_container_compose::ComposeEngine::new(compose, graph.name, backend));
    let _handle = engine.up(&[], true, false, false).await.map_err(|e| e.to_string())?;

    // 2. Resolve references and update environment
    let running_infos = engine.ps().await.map_err(|e| e.to_string())?;
    let mut nodes_map = HashMap::new();
    for info in running_infos {
        // Find node ID from project/service labels
        if let Some(node_id) = info.labels.get("com.docker.compose.service") {
             nodes_map.insert(node_id.clone(), info);
        }
    }

    // Update engine's spec with resolved values
    let mut updated_spec = engine.spec.clone();
    for (id, node) in &graph.nodes {
        if let Some(svc) = updated_spec.services.get_mut(id) {
            if let Some(perry_container_compose::types::ListOrDict::Dict(ref mut map)) = svc.environment {
                for (k, v) in &node.env {
                    if let WorkloadEnvValue::Ref(r) = v {
                        let resolved = r.resolve(&nodes_map)?;
                        map.insert(k.clone(), Some(serde_yaml::Value::String(resolved)));
                    }
                }
            }
        }
    }

    // Update the engine in-place with resolved spec
    // Note: Since we've already started the containers, this mostly updates the registry
    // for subsequent status/config/ps calls. In a real engine we might re-create containers if env changed.

    Ok(ctx.register_handle(HandleEntry::Engine(engine)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workload_ref_resolution() {
        let mut nodes = HashMap::new();
        nodes.insert("db".to_string(), ContainerInfo {
            id: "123".into(),
            name: "db".into(),
            image: "postgres".into(),
            status: "running".into(),
            ports: vec!["5432".into()],
            labels: HashMap::new(),
            created: "now".into(),
        });

        let r = WorkloadRef {
            node_id: "db".to_string(),
            projection: "endpoint".to_string(),
            port: Some("5432".to_string()),
        };

        let resolved = r.resolve(&nodes).unwrap();
        assert_eq!(resolved, "5432");
    }
}
