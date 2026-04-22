//! `ComposeEngine` — the core compose orchestration engine.
//!
//! Provides `ComposeEngine::up()`, `down()`, `ps()`, `logs()`, `exec()`, etc.
//! Uses Kahn's algorithm for dependency resolution.

use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Global registry of running compose engines, keyed by stack ID.
static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

/// Next available stack ID
static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

/// The compose orchestration engine.
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    /// Resources that were created in this session
    session_containers: std::sync::Mutex<Vec<String>>,
    session_networks: std::sync::Mutex<Vec<String>>,
    session_volumes: std::sync::Mutex<Vec<String>>,
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
            session_containers: std::sync::Mutex::new(Vec::new()),
            session_networks: std::sync::Mutex::new(Vec::new()),
            session_volumes: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Register this engine in the global registry and return a handle.
    fn register(self: Arc<Self>) -> ComposeHandle {
        let stack_id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services: Vec<String> = self.spec.services.keys().cloned().collect();
        let handle = ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services,
        };
        COMPOSE_ENGINES
            .lock()
            .unwrap()
            .insert(stack_id, Arc::clone(&self));
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

    // ============ up / start ============

    /// Bring up services in dependency order.
    ///
    /// Creates networks and volumes first, then starts containers.
    /// On failure, rolls back all resources created during this session.
    pub async fn up(
        self: Arc<Self>,
        services: &[String],
        _detach: bool,
        build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // Filter to target services
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        // 1. Create networks (skip external)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt
                    .as_ref()
                    .map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = net_config_opt
                    .as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(net_name.as_str());

                // State-aware: only create if not exists
                if self.backend.inspect_network(resolved_name).await.is_err() {
                    let spec_config = net_config_opt.clone().unwrap_or_default();
                    let config = crate::backend::NetworkConfig {
                        driver: spec_config.driver,
                        labels: spec_config.labels.map(|l| l.to_map()).unwrap_or_default(),
                        internal: spec_config.internal.unwrap_or(false),
                        enable_ipv6: spec_config.enable_ipv6.unwrap_or(false),
                    };
                    tracing::info!("Creating network '{}'…", resolved_name);
                    if let Err(e) = self.backend.create_network(resolved_name, &config).await {
                        self.rollback().await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: format!("network/{}", net_name),
                            message: e.to_string(),
                        });
                    }
                    self.session_networks.lock().unwrap().push(resolved_name.to_string());
                }
            }
        }

        // 2. Create volumes (skip external)
        if let Some(volumes) = &self.spec.volumes {
            for (vol_name, vol_config_opt) in volumes {
                let external = vol_config_opt
                    .as_ref()
                    .map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = vol_config_opt
                    .as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(vol_name.as_str());

                // State-aware: only create if not exists
                let spec_config = vol_config_opt.clone().unwrap_or_default();
                let config = crate::backend::VolumeConfig {
                    driver: spec_config.driver,
                    labels: spec_config.labels.map(|l| l.to_map()).unwrap_or_default(),
                };
                tracing::info!("Creating volume '{}'…", resolved_name);
                if let Err(e) = self.backend.create_volume(resolved_name, &config).await {
                    self.rollback().await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: format!("volume/{}", vol_name),
                        message: e.to_string(),
                    });
                }
                self.session_volumes.lock().unwrap().push(resolved_name.to_string());
            }
        }

        // 3. Start services in dependency order
        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);
            let inspect_result = self.backend.inspect(&container_name).await;

            let res = match inspect_result {
                Ok(info) if info.status == "running" => Ok(()),
                Ok(info) if info.status != "not found" => {
                    self.backend.start(&container_name).await.map(|_| {
                        self.session_containers.lock().unwrap().push(container_name.clone());
                    })
                }
                _ => {
                    // Build if needed
                    if build && svc.needs_build() {
                        let build_config = svc.build.as_ref().unwrap().as_build();
                        let tag = svc.image_ref(svc_name);
                        tracing::info!("Building image '{}'…", tag);
                        if let Err(e) = self.backend.build(&build_config, &tag).await {
                            Err(e)
                        } else {
                            self.run_service(svc, svc_name, &container_name).await
                        }
                    } else {
                        // Check if image exists, if not and image_ref is set, try to pull
                        let image = svc.image_ref(svc_name);
                        if self.backend.list_images().await.map_or(true, |list| !list.iter().any(|i| i.repository == image || i.id == image)) {
                            if let Some(img) = &svc.image {
                                tracing::info!("Pulling image '{}'…", img);
                                if let Err(e) = self.backend.pull_image(img).await {
                                    return Err(ComposeError::ImagePullFailed { message: e.to_string() });
                                }
                            }
                        }
                        self.run_service(svc, svc_name, &container_name).await
                    }
                }
            };

            if let Err(e) = res {
                self.rollback().await;
                return Err(ComposeError::ServiceStartupFailed {
                    service: svc_name.clone(),
                    message: e.to_string(),
                });
            }
        }

        // Register and return handle
        Ok(self.register())
    }

    async fn run_service(&self, svc: &crate::types::ComposeService, svc_name: &str, container_name: &str) -> Result<()> {
        let image = svc.image_ref(svc_name);
        let env = svc.resolved_env();
        let ports = svc.port_strings();
        let vols = svc.volume_strings();

        let mut all_labels: HashMap<String, String> = svc
            .labels
            .as_ref()
            .map(|l| l.to_map())
            .unwrap_or_default();
        all_labels.insert("perry.compose.project".into(), self.project_name.clone());
        all_labels.insert("perry.compose.service".into(), svc_name.to_string());

        let cmd = svc.command_list();

        let spec = ContainerSpec {
            image: image.clone(),
            name: Some(container_name.to_string()),
            ports: Some(ports),
            volumes: Some(vols),
            env: Some(env),
            labels: Some(all_labels),
            cmd,
            rm: Some(false),
            read_only: svc.read_only,
            ..Default::default()
        };

        self.backend.run(&spec).await.map(|_| {
            self.session_containers.lock().unwrap().push(container_name.to_string());
        })
    }

    async fn rollback(&self) {
        tracing::info!("Rolling back session resources…");

        let containers = {
            let mut guard = self.session_containers.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for container_name in containers.iter().rev() {
            let _ = self.backend.stop(container_name, None).await;
            let _ = self.backend.remove(container_name, true).await;
        }

        let networks = {
            let mut guard = self.session_networks.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for net_name in networks {
            let _ = self.backend.remove_network(&net_name).await;
        }

        let volumes = {
            let mut guard = self.session_volumes.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for vol_name in volumes {
            let _ = self.backend.remove_volume(&vol_name).await;
        }
    }

    // ============ down / stop ============

    /// Stop and remove services in reverse dependency order.
    pub async fn down(
        &self,
        services: &[String],
        _remove_orphans: bool,
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
        if services.is_empty() {
            // Remove by project labels if no specific services targeted
            let all = self.backend.list(true).await?;
            for container in all {
                if container.labels.get("perry.compose.project").map(|v| v == &self.project_name).unwrap_or(false) {
                    if container.status == "running" {
                        let _ = self.backend.stop(&container.id, None).await;
                    }
                    let _ = self.backend.remove(&container.id, true).await;
                }
            }
        } else {
            for svc_name in &target {
                let svc = self
                    .spec
                    .services
                    .get(*svc_name)
                    .ok_or_else(|| ComposeError::NotFound((*svc_name).clone()))?;

                let container_name = service::service_container_name(svc, svc_name);
                let inspect_result = self.backend.inspect(&container_name).await;

                if let Ok(info) = inspect_result {
                    if info.status == "running" {
                        self.backend.stop(&container_name, None).await?;
                    }
                    self.backend.remove(&container_name, true).await?;
                }
            }
        }
        // Also clear session containers if they match target
        if services.is_empty() {
             let mut guard = self.session_containers.lock().unwrap();
             guard.clear();
        } else {
            let mut guard = self.session_containers.lock().unwrap();
            guard.retain(|c| !target.iter().any(|svc_name| {
                 if let Some(svc) = self.spec.services.get(*svc_name) {
                     service::service_container_name(svc, svc_name) == *c
                 } else {
                     false
                 }
            }));
        }

        // 2. Remove networks (non-external)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt
                    .as_ref()
                    .map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = net_config_opt
                    .as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(net_name.as_str());

                let _ = self.backend.remove_network(resolved_name).await;
            }
        }
        self.session_networks.lock().unwrap().clear();

        // 3. Remove volumes (if requested)
        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for (vol_name, vol_config_opt) in volumes {
                    let external = vol_config_opt
                        .as_ref()
                        .map_or(false, |c| c.external.unwrap_or(false));
                    if external {
                        continue;
                    }
                    let resolved_name = vol_config_opt
                        .as_ref()
                        .and_then(|c| c.name.as_deref())
                        .unwrap_or(vol_name.as_str());

                    let _ = self.backend.remove_volume(resolved_name).await;
                }
            }
            self.session_volumes.lock().unwrap().clear();
        }

        Ok(())
    }

    // ============ ps ============

    /// List the status of all services.
    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();

        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, svc_name);
            let info = match self.backend.inspect(&container_name).await {
                Ok(mut info) => {
                    info.ports = svc.port_strings();
                    info
                }
                Err(_) => ContainerInfo {
                    id: container_name.clone(),
                    name: container_name,
                    image: svc.image_ref(svc_name),
                    status: "not found".to_string(),
                    ports: svc.port_strings(),
                    labels: HashMap::new(),
                    created: String::new(),
                },
            };
            results.push(info);
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
    ) -> Result<HashMap<String, ContainerLogs>> {
        let service_names: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };

        let mut all_logs = HashMap::new();
        for svc_name in service_names {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);
            let logs = self.backend.logs(&container_name, tail).await?;
            all_logs.insert(svc_name.clone(), logs);
        }

        Ok(all_logs)
    }

    // ============ exec ============

    /// Execute a command in a running service container.
    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let svc = self
            .spec
            .services
            .get(service)
            .ok_or_else(|| ComposeError::NotFound(service.to_owned()))?;

        let container_name = service::service_container_name(svc, service);
        let info = self.backend.inspect(&container_name).await?;

        if info.status != "running" {
            return Err(ComposeError::ServiceStartupFailed {
                service: service.to_owned(),
                message: format!("container '{}' is not running", container_name),
            });
        }

        self.backend
            .exec(&container_name, cmd, None, None)
            .await
    }

    // ============ config ============

    /// Validate and return the resolved compose configuration.
    pub fn config(&self) -> Result<String> {
        self.spec.to_yaml()
    }

    /// Resolve the startup order of services using Kahn's algorithm.
    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        resolve_startup_order(&self.spec)
    }

    pub async fn status(&self) -> Result<crate::types::StackStatus> {
        let containers = self.ps().await?;
        let mut services = Vec::new();
        let mut healthy = true;

        for info in containers {
            let state = match info.status.as_str() {
                "running" => "running",
                "stopped" | "exited" => "stopped",
                "not found" => "unknown",
                _ => "pending",
            };
            if state != "running" {
                healthy = false;
            }
            services.push(crate::types::ServiceStatus {
                service: info.name.clone(),
                state: state.to_string(),
                container_id: Some(info.id),
                error: None,
            });
        }

        Ok(crate::types::StackStatus { services, healthy })
    }

    pub fn graph(&self) -> Result<crate::types::ServiceGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for (name, svc) in &self.spec.services {
            nodes.push(name.clone());
            if let Some(deps) = &svc.depends_on {
                for dep in deps.service_names() {
                    edges.push(crate::types::ServiceEdge {
                        from: dep,
                        to: name.clone(),
                    });
                }
            }
        }

        Ok(crate::types::ServiceGraph { nodes, edges })
    }
}

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run(&self, graph_json: &str, _opts_json: &str) -> Result<u64> {
        let spec: ComposeSpec = serde_json::from_str(graph_json).map_err(ComposeError::JsonError)?;
        let engine = ComposeEngine::new(spec, "workload".to_string(), self.backend.clone());
        let handle = Arc::new(engine).up(&[], true, false, false).await?;
        Ok(handle.stack_id)
    }
}

impl ComposeEngine {
    /// Start existing stopped services.
    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(&svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, &svc_name);
            self.backend.start(&container_name).await?;
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
            let svc = self
                .spec
                .services
                .get(&svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, &svc_name);
            self.backend.stop(&container_name, None).await?;
        }

        Ok(())
    }

    /// Restart services.
    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await
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
