//! Workload graph types and management.

pub use perry_container_compose::workload::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphHandle {
    pub id: u64,
    pub graph: WorkloadGraph,
}
