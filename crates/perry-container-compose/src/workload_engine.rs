use crate::error::{ComposeError, Result};
use crate::backend::ContainerBackend;
use crate::types::{ContainerSpec, ContainerInfo, IsolationLevel};
use crate::workload_types::*;
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use indexmap::IndexMap;

/// Global registry of running workload engines
static WORKLOAD_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<WorkloadGraphEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

static NEXT_WORKLOAD_ID: AtomicU64 = AtomicU64::new(1);

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
    pub graph: WorkloadGraph,
    pub started_ids: std::sync::Mutex<Vec<String>>,
    pub running_containers: std::sync::Mutex<HashMap<String, ContainerInfo>>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>, graph: WorkloadGraph) -> Self {
        Self {
            backend,
            graph,
            started_ids: std::sync::Mutex::new(Vec::new()),
            running_containers: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn run(
        backend: Arc<dyn ContainerBackend>,
        graph: WorkloadGraph,
        opts: RunGraphOptions,
    ) -> Result<u64> {
        let engine = Arc::new(Self::new(backend, graph));

        // 1. Resolve topological order
        let order = engine.resolve_order()?;

        for node_id in &order {
            let node = engine.graph.nodes.get(node_id).unwrap();

            // Apply policy
            let spec = engine.apply_policy(node)?;

            // Start container
            match engine.backend.run(&spec).await {
                Ok(handle) => {
                    let info = engine.backend.inspect(&handle.id).await?;
                    engine.started_ids.lock().unwrap().push(node_id.clone());
                    engine.running_containers.lock().unwrap().insert(node_id.clone(), info);
                }
                Err(e) => {
                    match opts.on_failure {
                        FailureStrategy::RollbackAll => {
                            engine.rollback().await;
                            return Err(e);
                        }
                        FailureStrategy::HaltGraph => break,
                        FailureStrategy::PartialContinue => continue,
                    }
                }
            }
        }

        let id = NEXT_WORKLOAD_ID.fetch_add(1, Ordering::SeqCst);
        WORKLOAD_ENGINES.lock().unwrap().insert(id, engine);
        Ok(id)
    }

    fn resolve_order(&self) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for id in self.graph.nodes.keys() {
            in_degree.insert(id.clone(), 0);
            dependents.insert(id.clone(), Vec::new());
        }

        for (id, node) in &self.graph.nodes {
            for dep in &node.depends_on {
                if !self.graph.nodes.contains_key(dep) {
                    return Err(ComposeError::validation(format!("Node {} depends on {} which is not in graph", id, dep)));
                }
                *in_degree.get_mut(id).unwrap() += 1;
                dependents.get_mut(dep).unwrap().push(id.clone());
            }
        }

        let mut queue: std::collections::BTreeSet<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(id) = queue.pop_first() {
            order.push(id.clone());
            for dep in dependents.get(&id).unwrap() {
                let deg = in_degree.get_mut(dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.insert(dep.clone());
                }
            }
        }

        if order.len() != self.graph.nodes.len() {
            let cycle_services: Vec<String> = in_degree
                .iter()
                .filter(|(_, &deg)| deg > 0)
                .map(|(id, _)| id.clone())
                .collect();
            return Err(ComposeError::DependencyCycle { services: cycle_services });
        }

        Ok(order)
    }

    fn apply_policy(&self, node: &WorkloadNode) -> Result<ContainerSpec> {
        let mut spec = ContainerSpec {
            image: node.image.clone().unwrap_or_default(),
            name: Some(node.name.clone()),
            ports: Some(node.ports.clone()),
            ..Default::default()
        };

        match node.policy.tier {
            PolicyTier::Untrusted => {
                spec.isolation_level = Some(IsolationLevel::MicroVm);
                spec.read_only = Some(true);
                spec.network = Some("none".to_string());
            }
            PolicyTier::Hardened => {
                spec.read_only = Some(true);
            }
            PolicyTier::Isolated => {
                spec.network = Some("none".to_string());
            }
            PolicyTier::Default => {}
        }

        if node.policy.no_network {
            spec.network = Some("none".to_string());
        }
        if node.policy.read_only_root {
            spec.read_only = Some(true);
        }

        Ok(spec)
    }

    async fn rollback(&self) {
        let ids = self.started_ids.lock().unwrap().clone();
        let containers = self.running_containers.lock().unwrap();

        for node_id in ids.iter().rev() {
            if let Some(info) = containers.get(node_id) {
                let _ = self.backend.stop(&info.id, None).await;
                let _ = self.backend.remove(&info.id, true).await;
            }
        }
    }
}
