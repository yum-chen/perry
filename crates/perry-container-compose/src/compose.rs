use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
        }
    }

    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<()> {
        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let net_cfg = if let Some(c) = config {
                    NetworkConfig {
                        driver: c.driver.clone(),
                        labels: c.labels.as_ref().map(|l| l.to_map()).unwrap_or_default(),
                        internal: c.internal.unwrap_or(false),
                        enable_ipv6: c.enable_ipv6.unwrap_or(false),
                    }
                } else {
                    NetworkConfig::default()
                };
                self.backend.create_network(name, &net_cfg).await?;
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let vol_cfg = if let Some(c) = config {
                    VolumeConfig {
                        driver: c.driver.clone(),
                        labels: c.labels.as_ref().map(|l| l.to_map()).unwrap_or_default(),
                    }
                } else {
                    VolumeConfig::default()
                };
                self.backend.create_volume(name, &vol_cfg).await?;
            }
        }

        // 3. Resolve order and start services
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        let mut started = Vec::new();
        for svc_name in target {
            let svc_spec = self.spec.services.get(svc_name).unwrap();
            let svc = service::Service::new(svc_name.clone(), svc_spec);

            match crate::orchestrate::orchestrate_service(&svc, self.backend.as_ref()).await {
                Ok(_) => {
                    started.push(svc);
                }
                Err(e) => {
                    // Rollback
                    for s in started.iter().rev() {
                        let _ = crate::orchestrate::stop_service(s, self.backend.as_ref()).await;
                        let _ = crate::orchestrate::remove_service(s, self.backend.as_ref()).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    pub async fn down(
        &self,
        services: &[String],
        _remove_orphans: bool,
        remove_volumes: bool,
    ) -> Result<()> {
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        for svc_name in target.iter().rev() {
            let svc = self.spec.services.get(*svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            let _ = self.backend.stop(&container_name, Some(10)).await;
            let _ = self.backend.remove(&container_name, true).await;
        }

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let _ = self.backend.remove_network(name).await;
            }
        }

        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for name in volumes.keys() {
                    let _ = self.backend.remove_volume(name).await;
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut infos = Vec::new();
        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(info) = self.backend.inspect(&container_name).await {
                infos.push(info);
            }
        }
        Ok(infos)
    }

    pub async fn logs(
        &self,
        services: &[String],
        tail: Option<u32>,
        follow: bool,
    ) -> Result<HashMap<String, String>> {
        if follow {
            // follow mode only supports one service at a time for CLI simplicity
            let svc_name = services.first().ok_or_else(|| ComposeError::ValidationError {
                message: "follow mode requires a service name".into(),
            })?;
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.logs_follow(&container_name, tail).await?;
            return Ok(HashMap::new());
        }

        let mut all_logs = HashMap::new();
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };

        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(logs) = self.backend.logs(&container_name, tail).await {
                all_logs.insert(svc_name.clone(), format!("STDOUT:\n{}\nSTDERR:\n{}", logs.stdout, logs.stderr));
            }
        }
        Ok(all_logs)
    }

    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let svc = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.into()))?;
        let container_name = service::service_container_name(svc, service);
        self.backend.exec(&container_name, cmd, env, workdir).await
    }

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.stop(&container_name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await
    }

    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        resolve_startup_order(&self.spec)
    }
}

pub struct WorkloadGraphEngine {
    pub graph: crate::types::WorkloadGraph,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(graph: crate::types::WorkloadGraph, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { graph, project_name, backend }
    }

    pub async fn run(&self) -> Result<Arc<ComposeEngine>> {
        // Map WorkloadGraph to ComposeSpec internally for reuse
        let mut services = IndexMap::new();
        for (id, node) in &self.graph.nodes {
            let mut svc = crate::types::ComposeService::default();
            svc.image = node.image.clone();
            svc.ports = node.ports.as_ref().map(|p| p.iter().map(|s| crate::types::PortSpec::Short(serde_yaml::Value::String(s.clone()))).collect());
            svc.isolation_level = Some(node.isolation_level_from_policy());
            // TODO: map other fields
            services.insert(id.clone(), svc);
        }

        let spec = crate::types::ComposeSpec {
            name: Some(self.graph.name.clone()),
            services,
            ..Default::default()
        };

        let engine = Arc::new(ComposeEngine::new(spec, self.project_name.clone(), Arc::clone(&self.backend)));
        engine.up(&[], true, false, false).await?;
        Ok(engine)
    }
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut in_degree: IndexMap<String, usize> = IndexMap::new();
    let mut dependents: IndexMap<String, Vec<String>> = IndexMap::new();

    for name in spec.services.keys() {
        in_degree.insert(name.clone(), 0);
        dependents.insert(name.clone(), Vec::new());
    }

    for (name, service) in &spec.services {
        if let Some(deps) = &service.depends_on {
            for dep in deps.service_names() {
                if !spec.services.contains_key(&dep) {
                    return Err(ComposeError::ValidationError {
                        message: format!("Service '{}' depends on '{}' which is not defined", name, dep)
                    });
                }
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }

    let mut queue: std::collections::BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut order: Vec<String> = Vec::new();
    while let Some(service) = queue.pop_first() {
        order.push(service.clone());
        for dependent in dependents.get(&service).unwrap_or(&Vec::new()).clone() {
            let deg = in_degree.get_mut(&dependent).unwrap();
            *deg -= 1;
            if *deg == 0 {
                queue.insert(dependent);
            }
        }
    }

    if order.len() != spec.services.len() {
        let cycle_services: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(name, _)| name.clone())
            .collect();
        return Err(ComposeError::DependencyCycle { services: cycle_services });
    }

    Ok(order)
}
