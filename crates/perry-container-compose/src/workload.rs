use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ComposeService, DependsOnSpec};
use std::collections::HashMap;
use std::sync::Arc;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub ports: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub depends_on: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
}

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run_graph(&self, graph: &WorkloadGraph) -> Result<()> {
        let mut services = IndexMap::new();
        for (id, node) in &graph.nodes {
            let mut svc = ComposeService::default();
            svc.image = node.image.clone();
            if let Some(ports) = &node.ports {
                svc.ports = Some(ports.iter().map(|p| crate::types::PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect());
            }
            if let Some(deps) = &node.depends_on {
                svc.depends_on = Some(DependsOnSpec::List(deps.clone()));
            }
            if let Some(env) = &node.env {
                let mut dict = IndexMap::new();
                for (k, v) in env {
                    dict.insert(k.clone(), Some(serde_yaml::Value::String(v.clone())));
                }
                svc.environment = Some(crate::types::ListOrDict::Dict(dict));
            }
            services.insert(id.clone(), svc);
        }

        let spec = ComposeSpec {
            name: Some(graph.name.clone()),
            services,
            ..Default::default()
        };

        let engine = crate::compose::ComposeEngine::new(spec, graph.name.clone(), self.backend.clone());
        engine.up(&[], true, false, false).await?;
        Ok(())
    }
}
