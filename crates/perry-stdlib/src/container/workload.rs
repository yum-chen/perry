use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::types::ContainerInfo;
use perry_container_compose::error::ComposeError;
use perry_container_compose::types::{RefProjection, WorkloadRef};

/// Task 14.2: Implementation of WorkloadRef resolution
pub fn resolve_workload_ref(
    workload_ref: &WorkloadRef,
    running_nodes: &HashMap<String, ContainerInfo>
) -> Result<String, ComposeError> {
    let node = running_nodes.get(&workload_ref.node_id)
        .ok_or_else(|| ComposeError::ValidationError {
            message: format!("Node '{}' not found in running set", workload_ref.node_id)
        })?;

    match workload_ref.projection {
        RefProjection::Endpoint => {
            let port = workload_ref.port.as_deref().unwrap_or("80");
            // Placeholder for host_ip:host_port from node.ports
            Ok(format!("{}:{}", node.name, port))
        }
        RefProjection::Ip => {
            Ok(node.name.clone()) // Placeholder for actual IP
        }
        RefProjection::InternalUrl => {
            let port = workload_ref.port.as_deref().unwrap_or("80");
            Ok(format!("http://{}:{}", node.name, port))
        }
    }
}
