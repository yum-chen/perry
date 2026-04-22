//! `ComposeEngine` — the core compose orchestration engine.
//!
//! Provides `ComposeEngine::up()`, `down()`, `ps()`, `logs()`, `exec()`, etc.
//! Uses Kahn's algorithm for dependency resolution.

use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs,
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
        let engines = COMPOSE_ENGINES.lock().unwrap();
        // Note: can't insert self while holding a reference, so we re-acquire later
        drop(engines);
        COMPOSE_ENGINES
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

    // ============ up / start ============

    /// Bring up services in dependency order.
    ///
    /// Creates networks and volumes first, then starts containers.
    /// On failure, rolls back all previously started containers.
    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        _build: bool,
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
        let mut created_networks = Vec::new();
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

                let spec_config = net_config_opt.clone().unwrap_or_default();
                let config = crate::backend::NetworkConfig {
                    driver: spec_config.driver,
                    labels: spec_config.labels.map(|l| l.to_map()).unwrap_or_default(),
                    internal: spec_config.internal.unwrap_or(false),
                    enable_ipv6: spec_config.enable_ipv6.unwrap_or(false),
                };
                tracing::info!("Creating network '{}'…", resolved_name);
                if let Err(e) = self.backend.create_network(resolved_name, &config).await {
                    self.rollback(&[], &created_networks, &[]).await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: format!("network/{}", net_name),
                        message: e.to_string(),
                    });
                }
                created_networks.push(resolved_name.to_string());
            }
        }

        // 2. Create volumes (skip external)
        let mut created_volumes = Vec::new();
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

                let spec_config = vol_config_opt.clone().unwrap_or_default();
                let config = crate::backend::VolumeConfig {
                    driver: spec_config.driver,
                    labels: spec_config.labels.map(|l| l.to_map()).unwrap_or_default(),
                };
                tracing::info!("Creating volume '{}'…", resolved_name);
                if let Err(e) = self.backend.create_volume(resolved_name, &config).await {
                    self.rollback(&[], &created_networks, &created_volumes).await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: format!("volume/{}", vol_name),
                        message: e.to_string(),
                    });
                }
                created_volumes.push(resolved_name.to_string());
            }
        }

        // 3. Start services in dependency order
        let mut started = Vec::new();

        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            if let Err(e) = crate::orchestrate::orchestrate_service(svc, svc_name, &*self.backend).await {
                self.rollback(&started, &created_networks, &created_volumes).await;
                return Err(ComposeError::ServiceStartupFailed {
                    service: svc_name.clone(),
                    message: e.to_string(),
                });
            }
            started.push(service::service_container_name(svc, svc_name));
        }

        // Record started containers
        self.started_containers.lock().unwrap().extend(started);

        // Register and return handle
        Ok(self.register())
    }


    async fn rollback(&self, started: &[String], networks: &[String], volumes: &[String]) {
        tracing::info!("Rolling back partial startup…");
        // Stop and remove containers in reverse order
        for container_name in started.iter().rev() {
            let _ = self.backend.stop(container_name, None).await;
            let _ = self.backend.remove(container_name, true).await;
        }
        // Remove networks
        for net_name in networks {
            let _ = self.backend.remove_network(net_name).await;
        }
        // Remove volumes
        for vol_name in volumes {
            let _ = self.backend.remove_volume(vol_name).await;
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
        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);
            let inspect_result = self.backend.inspect(&container_name).await;

            if let Ok(info) = inspect_result {
                if info.status == "running" {
                    self.backend.stop(&container_name, None).await?;
                }
                self.backend.remove(&container_name, true).await?;
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

    // ============ start / stop / restart ============

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

#[cfg(test)]
mod tests_v4 {
    use super::*;
    use proptest::prelude::*;

    // Feature: alloy-container, Property 9: Container name generation uniqueness
    proptest! {
        #[test]
        fn test_container_name_uniqueness(img1 in ".*", svc1 in ".*", img2 in ".*", svc2 in ".*") {
            prop_assume!(img1 != img2 || svc1 != svc2);
            let n1 = service::generate_name(&img1, &svc1);
            let n2 = service::generate_name(&img2, &svc2);
            // Due to random suffix, they should be different even if img/svc are same,
            // but we explicitly assume different inputs here to test general case.
            assert_ne!(n1, n2);
        }
    }
}

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run(&self, spec_json: &str, _opts_json: &str) -> Result<u64> {
        // In a real implementation, parse spec_json to WorkloadGraph,
        // convert to ComposeSpec, then call ComposeEngine::up.
        let spec: ComposeSpec = serde_json::from_str(spec_json).map_err(ComposeError::JsonError)?;
        let engine = ComposeEngine::new(spec, "workload".to_string(), Arc::clone(&self.backend));
        let handle = engine.up(&[], true, false, false).await?;
        Ok(handle.stack_id)
    }
}
