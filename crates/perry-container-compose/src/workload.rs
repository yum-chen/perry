use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::types::{ContainerSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadRuntime {
    Oci,
    Microvm,
    Wasm,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkloadPolicy {
    pub no_network: bool,
    pub read_only_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadNode {
    pub name: String,
    pub image: String,
    pub ports: Vec<String>,
    pub env: HashMap<String, String>,
    pub depends_on: Vec<String>,
    pub runtime: WorkloadRuntime,
    pub policy: WorkloadPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: HashMap<String, WorkloadNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnFailure {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOptions {
    pub strategy: ExecutionStrategy,
    pub on_failure: OnFailure,
}

#[derive(Clone)]
pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self { backend }
    }

    pub async fn run(&self, graph: &WorkloadGraph, options: &RunOptions) -> Result<GraphHandle> {
        let order = self.resolve_order(graph)?;
        let mut started = Vec::new();

        for name in order {
            let node = graph.nodes.get(&name).unwrap();
            let spec = ContainerSpec {
                image: node.image.clone(),
                name: Some(node.name.clone()),
                ports: Some(node.ports.clone()),
                env: Some(node.env.clone()),
                rm: Some(false),
                read_only: Some(node.policy.read_only_root),
                ..Default::default()
            };

            match self.backend.run(&spec).await {
                Ok(_) => started.push(name),
                Err(e) => {
                    if matches!(options.on_failure, OnFailure::RollbackAll) {
                        for s in started.iter().rev() {
                            let _ = self.backend.stop(s, None).await;
                            let _ = self.backend.remove(s, true).await;
                        }
                    }
                    return Err(e);
                }
            }
        }

        Ok(GraphHandle {
            id: rand::random(),
            graph: graph.clone(),
        })
    }

    fn resolve_order(&self, graph: &WorkloadGraph) -> Result<Vec<String>> {
        let mut in_degree = HashMap::new();
        let mut adj = HashMap::new();

        for (name, node) in &graph.nodes {
            in_degree.entry(name.clone()).or_insert(0);
            for dep in &node.depends_on {
                adj.entry(dep.clone()).or_insert_with(Vec::new).push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: std::collections::VecDeque<_> = in_degree
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(n, _)| n.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(u) = queue.pop_front() {
            order.push(u.clone());
            if let Some(neighbors) = adj.get(&u) {
                for v in neighbors {
                    let d = in_degree.get_mut(v).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(v.clone());
                    }
                }
            }
        }

        if order.len() != graph.nodes.len() {
            return Err(ComposeError::DependencyCycle {
                services: in_degree.keys().cloned().collect(),
            });
        }

        Ok(order)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphHandle {
    pub id: u64,
    pub graph: WorkloadGraph,
}
