//! `ComposeEngine` — the core compose orchestration engine.
//!
//! Provides `ComposeEngine::up()`, `down()`, `ps()`, `logs()`, `exec()`, etc.
//! Uses Kahn's algorithm for dependency resolution.

use crate::backend::ContainerBackend;
pub use crate::types::ContainerLogs;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerSpec,
};
use crate::backend::{NetworkConfig, VolumeConfig};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use serde::{Serialize, Deserialize};

/// Global registry of running compose engines, keyed by stack ID.
static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

/// Next available stack ID
static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

/// The resolved dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<ServiceEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEdge {
    pub from: String,
    pub to: String,
}

/// Status of a compose stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackStatus {
    pub services: Vec<ServiceStatus>,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub service: String,
    pub state: String,
    pub container_id: Option<String>,
    pub error: Option<String>,
}

/// The compose orchestration engine.
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    /// Services that were started in this session
    started_containers: std::sync::Mutex<Vec<String>>,
}

impl ComposeEngine {
    /// Create a new ComposeEngine.
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
            started_containers: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Register this engine in the global registry and return a handle.
    fn register(&self) -> ComposeHandle {
        let stack_id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services: Vec<String> = self.spec.services.keys().cloned().collect();
        let handle = ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services,
        };
        let _ = COMPOSE_ENGINES
            .lock()
            .unwrap()
            .insert(stack_id, Arc::new(ComposeEngine::new(
                self.spec.clone(),
                self.project_name.clone(),
                Arc::clone(&self.backend),
            )));
        handle
    }

    /// Look up an engine by stack ID.
    pub fn get_engine(stack_id: u64) -> Option<Arc<ComposeEngine>> {
        COMPOSE_ENGINES.lock().unwrap().get(&stack_id).cloned()
    }

    /// Remove an engine from the registry.
    pub fn unregister(stack_id: u64) {
        COMPOSE_ENGINES.lock().unwrap().shift_remove(&stack_id);
    }

    /// Method delegate for test compatibility.
    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        resolve_startup_order(&self.spec)
    }

    // ============ up / start ============

    /// Bring up services in dependency order.
    ///
    /// Creates networks and volumes first, then starts containers.
    /// On failure, rolls back all previously started containers.
    pub async fn up(
        &self,
        services: &[String],
        detach: bool,
        build: bool,
        remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // Filter to target services
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        // 0. Remove orphans if requested
        if remove_orphans {
            self.remove_orphans().await?;
        }

        let mut created_networks: Vec<String> = Vec::new();
        let mut created_volumes: Vec<String> = Vec::new();

        // 1. Create networks (skip external)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let net_config_spec = net_config_opt.as_ref().cloned().unwrap_or_default();
                let net_config = NetworkConfig::from(&net_config_spec);
                let resolved_name = net_config_spec.name.as_deref().unwrap_or(net_name.as_str());

                // Check if already exists
                if self.backend.inspect_network(resolved_name).await.is_err() {
                    tracing::info!("Creating network '{}'…", resolved_name);
                    if let Err(e) = self.backend.create_network(resolved_name, &net_config).await {
                        // Rollback created networks
                        for n in created_networks.iter().rev() {
                            let _ = self.backend.remove_network(n).await;
                        }
                        return Err(ComposeError::ServiceStartupFailed {
                            service: format!("network/{}", net_name),
                            message: e.to_string(),
                        });
                    }
                    created_networks.push(resolved_name.to_string());
                }
            }
        }

        // 2. Create volumes (skip external)
        if let Some(volumes) = &self.spec.volumes {
            for (vol_name, vol_config_opt) in volumes {
                let external = vol_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let vol_config_spec = vol_config_opt.as_ref().cloned().unwrap_or_default();
                let vol_config = VolumeConfig::from(&vol_config_spec);
                let resolved_name = vol_config_spec.name.as_deref().unwrap_or(vol_name.as_str());

                // Check if already exists
                if self.backend.inspect_volume(resolved_name).await.is_err() {
                    tracing::info!("Creating volume '{}'…", resolved_name);
                    if let Err(e) = self.backend.create_volume(resolved_name, &vol_config).await {
                        // Rollback created volumes and networks
                        for v in created_volumes.iter().rev() {
                            let _ = self.backend.remove_volume(v).await;
                        }
                        for n in created_networks.iter().rev() {
                            let _ = self.backend.remove_network(n).await;
                        }
                        return Err(ComposeError::ServiceStartupFailed {
                            service: format!("volume/{}", vol_name),
                            message: e.to_string(),
                        });
                    }
                    created_volumes.push(resolved_name.to_string());
                }
            }
        }

        // 3. Start services in dependency order
        let mut started: Vec<String> = Vec::new();

        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            // 2.5 Build image if requested or needed
            let image_ref = svc.image_ref(svc_name);
            if build || svc.needs_build() {
                if let Some(build_spec) = &svc.build {
                    let build_config = build_spec.as_build();
                    let context = build_config.context.as_deref().unwrap_or(".");
                    tracing::info!("Building service '{}' (image: {})...", svc_name, image_ref);
                    self.backend
                        .build_image(
                            context,
                            &image_ref,
                            build_config.dockerfile.as_deref(),
                            build_config.args.as_ref().map(|l| l.to_map()).as_ref(),
                        )
                        .await
                        .map_err(|e| ComposeError::ServiceStartupFailed {
                            service: svc_name.clone(),
                            message: format!("Build failed: {}", e),
                        })?;
                }
            }

            let container_name = service::service_container_name(svc, svc_name);

            // Check if already exists and running
            let info_res = self.backend.inspect(&container_name).await;

            let res = match info_res {
                Ok(info) if info.status == "running" => {
                    // Already running
                    Ok(())
                }
                Ok(_info) => {
                    // Exists but not running
                    self.backend.start(&container_name).await
                }
                Err(_) => {
                    // Does not exist
                    let mut labels = svc.labels.as_ref().map(|l| l.to_map()).unwrap_or_default();
                    labels.insert("com.docker.compose.project".into(), self.project_name.clone());
                    labels.insert("com.docker.compose.service".into(), svc_name.clone());

                    let spec = ContainerSpec {
                        image: svc.image_ref(svc_name),
                        name: Some(container_name.clone()),
                        ports: Some(svc.port_strings()),
                        volumes: Some(svc.volume_strings()),
                        env: Some(svc.resolved_env()),
                        labels: Some(labels),
                        cmd: svc.command_list(),
                        rm: Some(false),
                        ..Default::default()
                    };

                    if detach {
                        self.backend.run(&spec).await.map(|_| ())
                    } else {
                        match self.backend.create(&spec).await {
                            Ok(_) => self.backend.start(&container_name).await,
                            Err(e) => Err(e),
                        }
                    }
                }
            };

            if let Err(e) = res {
                // ROLLBACK
                tracing::error!("Service '{}' failed to start, rolling back...", svc_name);
                for c_name in started.iter().rev() {
                    let _ = self.backend.stop(c_name, None).await;
                    let _ = self.backend.remove(c_name, true).await;
                }
                // Rollback created volumes and networks
                for v in created_volumes.iter().rev() {
                    let _ = self.backend.remove_volume(v).await;
                }
                for n in created_networks.iter().rev() {
                    let _ = self.backend.remove_network(n).await;
                }
                return Err(ComposeError::ServiceStartupFailed {
                    service: svc_name.clone(),
                    message: e.to_string(),
                });
            }

            started.push(container_name.clone());
        }

        // Record started containers
        self.started_containers.lock().unwrap().extend(started);

        // Register and return handle
        Ok(self.register())
    }

    /// Remove containers that are not defined in the current spec but
    /// have the project name label.
    async fn remove_orphans(&self) -> Result<()> {
        let containers = self.backend.list(true).await?;

        for container in containers {
            if let Some(project) = container.labels.get("com.docker.compose.project") {
                if project == &self.project_name {
                    if let Some(service) = container.labels.get("com.docker.compose.service") {
                        if !self.spec.services.contains_key(service) {
                            tracing::info!("Removing orphan container '{}' (service: {})...", container.name, service);
                            if container.status.contains("running") {
                                let _ = self.backend.stop(&container.id, None).await;
                            }
                            let _ = self.backend.remove(&container.id, true).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Find a container for a service by project and service labels.
    async fn find_container_for_service(&self, svc_name: &str) -> Result<Option<ContainerInfo>> {
        let containers = self.backend.list(true).await?;
        for c in containers {
            let p = c.labels.get("com.docker.compose.project");
            let s = c.labels.get("com.docker.compose.service");
            if p.map(|v| v == &self.project_name).unwrap_or(false)
                && s.map(|v| v == svc_name).unwrap_or(false)
            {
                return Ok(Some(c));
            }
        }
        Ok(None)
    }

    // ============ down / stop ============

    /// Stop and remove services in reverse dependency order.
    pub async fn down(
        &self,
        services: &[String],
        remove_orphans: bool,
        remove_volumes: bool,
    ) -> Result<()> {
        let mut order = resolve_startup_order(&self.spec)?;
        order.reverse();

        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        // 1. Stop and remove containers
        for svc_name in target {
            let container_info = self.find_container_for_service(svc_name).await?;

            if let Some(info) = container_info {
                tracing::info!("Stopping and removing container '{}'...", info.name);
                if info.status.contains("running") || info.status.contains("Up") {
                    let _ = self.backend.stop(&info.id, None).await;
                }
                self.backend.remove(&info.id, true).await?;
            } else {
                tracing::warn!("No container found for service '{}', skipping", svc_name);
            }
        }

        // 2. Remove networks (non-external, idempotent)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = net_config_opt.as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(net_name.as_str());
                let _ = self.backend.remove_network(resolved_name).await;
            }
        }

        // 2.5 Remove orphans if requested
        if remove_orphans {
            self.remove_orphans().await?;
        }

        // 3. Remove volumes (if requested)
        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for (vol_name, vol_config_opt) in volumes {
                    let external = vol_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                    if external {
                        continue;
                    }
                    let resolved_name = vol_config_opt.as_ref()
                        .and_then(|c| c.name.as_deref())
                        .unwrap_or(vol_name.as_str());
                    let _ = self.backend.remove_volume(resolved_name).await;
                }
            }
        }

        Ok(())
    }

    // ============ ps ============

    /// List the status of all services.
    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();

        for (svc_name, svc) in &self.spec.services {
            let container_info = self.find_container_for_service(svc_name).await?;

            match container_info {
                Some(info) => results.push(info),
                None => {
                    results.push(ContainerInfo {
                        id: "".into(),
                        name: "".into(),
                        image: svc.image_ref(svc_name),
                        status: "not found".to_string(),
                        ports: svc.port_strings(),
                        labels: HashMap::new(),
                        created: String::new(),
                    });
                }
            }
        }

        results.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(results)
    }

    // ============ logs ============

    /// Get logs from services.
    pub async fn logs(
        &self,
        services: &[String],
        tail: Option<u32>,
    ) -> Result<ContainerLogs> {
        let mut stdout = String::new();
        let mut stderr = String::new();

        let service_names: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in service_names {
            if let Some(info) = self.find_container_for_service(&svc_name).await? {
                let logs = self.backend.logs(&info.id, tail).await?;
                stdout.push_str(&format!("--- {} ---\n{}", svc_name, logs.stdout));
                stderr.push_str(&format!("--- {} ---\n{}", svc_name, logs.stderr));
            } else {
                stdout.push_str(&format!("--- {} ---\n(not found)\n", svc_name));
            }
        }

        Ok(ContainerLogs { stdout, stderr })
    }

    // ============ exec ============

    /// Execute a command in a running service container.
    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
    ) -> Result<ContainerLogs> {
        let info = self.find_container_for_service(service).await?
            .ok_or_else(|| ComposeError::NotFound(service.to_string()))?;

        if !info.status.contains("running") {
            return Err(ComposeError::ServiceStartupFailed {
                service: service.to_owned(),
                message: format!("container '{}' is not running", info.name),
            });
        }

        self.backend
            .exec(&info.id, cmd, None, None)
            .await
    }

    // ============ config ============

    /// Validate and return the resolved compose configuration.
    pub fn config(&self) -> Result<String> {
        self.spec.to_yaml()
    }

    // ============ start / stop / restart ============

    /// Start existing stopped services.
    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in target {
            if let Some(info) = self.find_container_for_service(&svc_name).await? {
                self.backend.start(&info.id).await?;
            } else {
                return Err(ComposeError::NotFound(svc_name));
            }
        }

        Ok(())
    }

    /// Stop running services.
    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in target {
            if let Some(info) = self.find_container_for_service(&svc_name).await? {
                self.backend.stop(&info.id, None).await?;
            } else {
                return Err(ComposeError::NotFound(svc_name));
            }
        }

        Ok(())
    }

    /// Restart services.
    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await
    }

    pub fn graph(&self) -> Result<ServiceGraph> {
        let order = resolve_startup_order(&self.spec)?;
        let mut edges = Vec::new();
        for (name, svc) in &self.spec.services {
            if let Some(deps) = &svc.depends_on {
                for dep in deps.service_names() {
                    edges.push(ServiceEdge { from: name.clone(), to: dep });
                }
            }
        }
        Ok(ServiceGraph { nodes: order, edges })
    }

    pub async fn status(&self) -> Result<StackStatus> {
        let mut services = Vec::new();
        let mut healthy = true;
        for svc_name in self.spec.services.keys() {
            let container_info = self.find_container_for_service(svc_name).await?;
            let (state, container_id) = match container_info {
                Some(info) => {
                    let st = info.status.clone();
                    if !st.contains("running") && !st.contains("Up") { healthy = false; }
                    (st, Some(info.id))
                }
                None => { healthy = false; ("not found".into(), None) }
            };
            services.push(ServiceStatus {
                service: svc_name.clone(),
                state,
                container_id,
                error: None,
            });
        }
        Ok(StackStatus { services, healthy })
    }
}

// ============ Dependency resolution (Kahn's algorithm) ============

/// Resolve the startup order of services using Kahn's algorithm (BFS topological sort).
///
/// Returns services in dependency order. If a cycle is detected, returns
/// `ComposeError::DependencyCycle` listing all services in the cycle.
pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    // 1. Build adjacency list and in-degrees
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
                    return Err(ComposeError::validation(format!(
                        "Service '{}' depends on '{}' which is not defined",
                        name, dep
                    )));
                }
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }

    // 2. Queue all services with in-degree 0 (sorted for determinism)
    let mut queue: std::collections::BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    // 3. Process queue
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

    // 4. If not all services processed → cycle detected
    if order.len() != spec.services.len() {
        let cycle_services: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(name, _)| name.clone())
            .collect();
        return Err(ComposeError::DependencyCycle {
            services: cycle_services,
        });
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ComposeService;

    fn make_compose(edges: &[(&str, &[&str])]) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (name, deps) in edges {
            let mut svc = ComposeService::default();
            if !deps.is_empty() {
                svc.depends_on = Some(crate::types::DependsOnSpec::List(
                    deps.iter().map(|s| s.to_string()).collect(),
                ));
            }
            services.insert(name.to_string(), svc);
        }
        ComposeSpec {
            services,
            ..Default::default()
        }
    }

    #[test]
    fn test_simple_chain() {
        let compose = make_compose(&[("web", &["db"]), ("db", &[]), ("proxy", &["web"])]);
        let order = resolve_startup_order(&compose).unwrap();
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("db") < pos("web"), "db must precede web");
        assert!(pos("web") < pos("proxy"), "web must precede proxy");
    }

    #[test]
    fn test_no_deps() {
        let compose = make_compose(&[("a", &[]), ("b", &[]), ("c", &[])]);
        let order = resolve_startup_order(&compose).unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_diamond_dependency() {
        // a -> b, a -> c, b -> d, c -> d
        let compose = make_compose(&[
            ("a", &[]),
            ("b", &["a"]),
            ("c", &["a"]),
            ("d", &["b", "c"]),
        ]);
        let order = resolve_startup_order(&compose).unwrap();
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("a") < pos("b"));
        assert!(pos("a") < pos("c"));
        assert!(pos("b") < pos("d"));
        assert!(pos("c") < pos("d"));
    }

    #[test]
    fn test_cycle_detected() {
        let compose = make_compose(&[("a", &["b"]), ("b", &["a"])]);
        let result = resolve_startup_order(&compose);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComposeError::DependencyCycle { .. }
        ));
    }

    #[test]
    fn test_cycle_lists_all_services() {
        // a -> b -> c -> a (3-node cycle)
        let compose = make_compose(&[("a", &["c"]), ("b", &["a"]), ("c", &["b"])]);
        let result = resolve_startup_order(&compose);
        assert!(result.is_err());
        if let ComposeError::DependencyCycle { services } = result.unwrap_err() {
            assert_eq!(services.len(), 3);
            assert!(services.contains(&"a".to_string()));
            assert!(services.contains(&"b".to_string()));
            assert!(services.contains(&"c".to_string()));
        }
    }

    #[test]
    fn test_invalid_dependency() {
        let compose = make_compose(&[("web", &["nonexistent"])]);
        let result = resolve_startup_order(&compose);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComposeError::ValidationError { .. }));
    }

    #[test]
    fn test_deterministic_order() {
        // Services with no deps should be sorted alphabetically
        let compose = make_compose(&[("c", &[]), ("a", &[]), ("b", &[])]);
        let order = resolve_startup_order(&compose).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;

    // Feature: alloy-container, Property 7: Topological sort produces valid ordering
    #[test]
    fn test_topological_sort_validity() {
        let mut spec = ComposeSpec::default();
        let svc_a = crate::types::ComposeService::default();
        let mut svc_b = crate::types::ComposeService::default();
        svc_b.depends_on = Some(crate::types::DependsOnSpec::List(vec!["a".into()]));
        spec.services.insert("a".into(), svc_a);
        spec.services.insert("b".into(), svc_b);

        let order = resolve_startup_order(&spec).unwrap();
        assert_eq!(order, vec!["a", "b"]);
    }

    // Feature: alloy-container, Property 8: Cycle detection is exhaustive
    #[test]
    fn test_cycle_detection_exhaustive() {
        let mut spec = ComposeSpec::default();
        let mut svc_a = crate::types::ComposeService::default();
        svc_a.depends_on = Some(crate::types::DependsOnSpec::List(vec!["b".into()]));
        let mut svc_b = crate::types::ComposeService::default();
        svc_b.depends_on = Some(crate::types::DependsOnSpec::List(vec!["a".into()]));
        spec.services.insert("a".into(), svc_a);
        spec.services.insert("b".into(), svc_b);

        let result = resolve_startup_order(&spec);
        assert!(result.is_err());
        if let Err(ComposeError::DependencyCycle { services }) = result {
            assert!(services.contains(&"a".into()));
            assert!(services.contains(&"b".into()));
        } else {
            panic!("Expected DependencyCycle error");
        }
    }

    // Feature: alloy-container, Property 9: Container name generation uniqueness
    #[test]
    fn test_container_name_uniqueness() {
        let svc = crate::types::ComposeService::default();
        let name1 = service::service_container_name(&svc, "svc1");
        let name2 = service::service_container_name(&svc, "svc2");
        assert_ne!(name1, name2);
    }
}
