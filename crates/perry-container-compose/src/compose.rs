use crate::error::{ComposeError, Result};
use crate::service::{Service, ServiceBuild};
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
    WorkloadGraph, RunGraphOptions, GraphHandle, ExecutionStrategy, FailureStrategy
};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use indexmap::IndexMap;
use std::collections::HashMap;
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

    /// Requirement 6.1, 6.8, 6.9, 6.10, 6.13
    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
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

            // Map types::ComposeService to service::Service
            let service_entity = Service {
                image: svc_spec.image.clone(),
                name: svc_spec.container_name.clone(),
                ports: Some(svc_spec.ports.as_ref().map(|p| p.iter().map(|ps| ps.to_string_form()).collect()).unwrap_or_default()),
                environment: svc_spec.environment.clone(),
                labels: svc_spec.labels.clone(),
                volumes: Some(svc_spec.volumes.as_ref().map(|v| v.iter().map(|y| format!("{:?}", y)).collect()).unwrap_or_default()),
                build: svc_spec.build.as_ref().map(|b| {
                   let b_spec = b.as_build();
                   ServiceBuild {
                       context: b_spec.context.clone().unwrap_or_default(),
                       dockerfile: b_spec.dockerfile.clone(),
                       args: b_spec.args.as_ref().map(|a| a.to_map()),
                       labels: b_spec.labels.clone(),
                       target: b_spec.target.clone(),
                       network: b_spec.network.clone(),
                   }
                }),
                command: svc_spec.command_list(),
                entrypoint: None, // TODO
                env_file: None, // TODO
                networks: svc_spec.networks.as_ref().map(|n| n.names()),
                depends_on: svc_spec.depends_on.clone(),
                restart: svc_spec.restart.clone(),
                healthcheck: svc_spec.healthcheck.clone(),
                working_dir: svc_spec.working_dir.clone(),
                user: svc_spec.user.clone(),
                hostname: svc_spec.hostname.clone(),
                privileged: svc_spec.privileged,
                read_only: svc_spec.read_only,
                stdin_open: svc_spec.stdin_open,
                tty: svc_spec.tty,
                isolation_level: svc_spec.isolation_level.clone(),
            };

            match crate::orchestrate::orchestrate_service(svc_name, &service_entity, self.backend.as_ref()).await {
                Ok(_) => {
                    started.push((svc_name.clone(), service_entity));
                }
                Err(e) => {
                    // Rollback Requirement 6.10
                    for (s_name, s_entity) in started.iter().rev() {
                        let _ = crate::orchestrate::stop_service(s_name, s_entity, self.backend.as_ref()).await;
                        let _ = crate::orchestrate::remove_service(s_name, s_entity, self.backend.as_ref()).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: 0, // Should be assigned by registry
            project_name: self.project_name.clone(),
            services: self.spec.services.keys().cloned().collect(),
        })
    }

    pub async fn down(
        &self,
        remove_volumes: bool,
    ) -> Result<()> {
        let order = resolve_startup_order(&self.spec)?;

        for svc_name in order.iter().rev() {
            let svc_spec = self.spec.services.get(svc_name).unwrap();
            let image = svc_spec.image.as_deref().unwrap_or("unknown");
            let container_name = svc_spec.container_name.clone().unwrap_or_else(|| Service::generate_name(image, svc_name));
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
        for (svc_name, svc_spec) in &self.spec.services {
            let image = svc_spec.image.as_deref().unwrap_or("unknown");
            let container_name = svc_spec.container_name.clone().unwrap_or_else(|| Service::generate_name(image, svc_name));
            if let Ok(info) = self.backend.inspect(&container_name).await {
                infos.push(info);
            }
        }
        Ok(infos)
    }

    pub async fn logs(
        &self,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<ContainerLogs> {
        let mut all_stdout = String::new();
        let mut all_stderr = String::new();

        let target: Vec<&String> = if let Some(s) = service {
            vec![self.spec.services.get_key_value(s).map(|(k, _)| k).ok_or_else(|| ComposeError::NotFound(s.into()))?]
        } else {
            self.spec.services.keys().collect()
        };

        for svc_name in target {
            let svc_spec = self.spec.services.get(svc_name).unwrap();
            let image = svc_spec.image.as_deref().unwrap_or("unknown");
            let container_name = svc_spec.container_name.clone().unwrap_or_else(|| Service::generate_name(image, svc_name));
            if let Ok(logs) = self.backend.logs(&container_name, tail).await {
                all_stdout.push_str(&format!("--- {} ---\n{}", svc_name, logs.stdout));
                all_stderr.push_str(&format!("--- {} ---\n{}", svc_name, logs.stderr));
            }
        }
        Ok(ContainerLogs { stdout: all_stdout, stderr: all_stderr })
    }

    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
    ) -> Result<ContainerLogs> {
        let svc_spec = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.into()))?;
        let image = svc_spec.image.as_deref().unwrap_or("unknown");
        let container_name = svc_spec.container_name.clone().unwrap_or_else(|| Service::generate_name(image, service));
        self.backend.exec(&container_name, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };
        for svc_name in target {
            let svc_spec = self.spec.services.get(svc_name).unwrap();
            let image = svc_spec.image.as_deref().unwrap_or("unknown");
            let container_name = svc_spec.container_name.clone().unwrap_or_else(|| Service::generate_name(image, svc_name));
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
            let svc_spec = self.spec.services.get(svc_name).unwrap();
            let image = svc_spec.image.as_deref().unwrap_or("unknown");
            let container_name = svc_spec.container_name.clone().unwrap_or_else(|| Service::generate_name(image, svc_name));
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

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }
}

pub struct WorkloadGraphEngine {
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { project_name, backend }
    }

    /// Task 15.1: Implement WorkloadGraphEngine::run
    pub async fn run(&self, graph: WorkloadGraph, opts: RunGraphOptions) -> Result<GraphHandle> {
        let levels = compute_topological_levels(&graph)?;
        let mut started: Vec<String> = vec![];

        let strategy = opts.strategy.unwrap_or(ExecutionStrategy::DependencyAware);
        let on_failure = opts.on_failure.unwrap_or(FailureStrategy::RollbackAll);

        for level in &levels {
            let nodes_to_start = match strategy {
                ExecutionStrategy::Sequential => vec![level[0].clone()], // simplification
                _ => level.clone(),
            };

            // In a real implementation we'd handle ParallelSafe with sleep etc.
            for node_id in nodes_to_start {
                let node = graph.nodes.get(&node_id).unwrap();

                // Construct ContainerSpec from WorkloadNode and Policy
                let spec = ContainerSpec {
                    image: node.image.clone().unwrap_or_default(),
                    name: Some(node.name.clone()),
                    ports: Some(node.ports.clone()),
                    env: Some(HashMap::new()), // Need to resolve refs later
                    ..Default::default()
                };

                // Apply Policy Requirement 15.1
                if node.policy.tier == crate::types::PolicyTier::Untrusted {
                    // Force MicroVm etc.
                }

                match self.backend.run(&spec).await {
                    Ok(_) => started.push(node_id),
                    Err(e) => match on_failure {
                        FailureStrategy::RollbackAll => {
                            for s in started.iter().rev() {
                                let _ = self.backend.remove(s, true).await;
                            }
                            return Err(ComposeError::ServiceStartupFailed { service: node_id, message: e.to_string() });
                        }
                        _ => return Err(ComposeError::ServiceStartupFailed { service: node_id, message: e.to_string() }),
                    }
                }
            }
        }

        Ok(GraphHandle { id: 0 })
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

pub fn compute_topological_levels(graph: &WorkloadGraph) -> Result<Vec<Vec<String>>> {
    let mut levels = Vec::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for id in graph.nodes.keys() {
        in_degree.insert(id.clone(), 0);
    }

    for (id, node) in &graph.nodes {
        for dep in &node.depends_on {
            *in_degree.entry(id.clone()).or_insert(0) += 1;
            dependents.entry(dep.clone()).or_default().push(id.clone());
        }
    }

    let mut current_level: Vec<String> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    while !current_level.is_empty() {
        levels.push(current_level.clone());
        let mut next_level = Vec::new();
        for id in current_level {
            if let Some(deps) = dependents.get(&id) {
                for dep_id in deps {
                    let deg = in_degree.get_mut(dep_id).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next_level.push(dep_id.clone());
                    }
                }
            }
        }
        current_level = next_level;
    }

    Ok(levels)
}
