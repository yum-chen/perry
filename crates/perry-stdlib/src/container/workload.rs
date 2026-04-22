use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::types::ContainerInfo;
use perry_container_compose::error::ComposeError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySpec {
    pub tier: String, // "default" | "isolated" | "hardened" | "untrusted"
    pub no_network: Option<bool>,
    pub read_only_root: Option<bool>,
    pub seccomp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: String, // "endpoint" | "ip" | "internal_url"
    pub port: Option<String>,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, ComposeError> {
        let node = running_nodes.get(&self.node_id)
            .ok_or_else(|| ComposeError::ValidationError {
                message: format!("Node '{}' not found in running set", self.node_id)
            })?;

        match self.projection.as_str() {
            "endpoint" => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("{}:{}", node.name, port)) // Simplified for now
            }
            "ip" => {
                // In a real implementation, we'd extract IP from ContainerInfo
                Ok(node.name.clone())
            }
            "internal_url" => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("http://{}:{}", node.name, port))
            }
            _ => Err(ComposeError::ValidationError { message: "Invalid projection".into() }),
        }
    }
}
