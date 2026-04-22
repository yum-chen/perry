use indexmap::IndexMap;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec, ComposeHandle, ContainerHandle, ListOrDict};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::service::generate_name;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self { spec, project_name, backend }
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

        let mut queue: BTreeSet<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut order: Vec<String> = Vec::new();
        while let Some(service) = queue.pop_first() {
            order.push(service.clone());
            if let Some(deps) = dependents.get(&service) {
                for dependent in deps {
                    let deg = in_degree.get_mut(dependent).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.insert(dependent.clone());
                    }
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

    pub async fn up(&self, _services: &[String], _detach: bool, _build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let order = Self::resolve_startup_order(&self.spec)?;
        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers: Vec<(String, ContainerHandle)> = Vec::new();

        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let config = config.clone().unwrap_or_default();
                let net_config = NetworkConfig {
                    driver: config.driver,
                    labels: match config.labels {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), format!("{:?}", v))).collect(),
                        _ => HashMap::new(),
                    },
                    internal: config.internal.unwrap_or(false),
                    enable_ipv6: config.enable_ipv6.unwrap_or(false),
                };
                let prefixed_name = format!("{}_{}", self.project_name, name);
                if let Err(e) = self.backend.create_network(&prefixed_name, &net_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(prefixed_name);
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let config = config.clone().unwrap_or_default();
                let vol_config = VolumeConfig {
                    driver: config.driver,
                    labels: match config.labels {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), format!("{:?}", v))).collect(),
                        _ => HashMap::new(),
                    },
                };
                let prefixed_name = format!("{}_{}", self.project_name, name);
                if let Err(e) = self.backend.create_volume(&prefixed_name, &vol_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_volumes.push(prefixed_name);
            }
        }

        for service_name in order {
            let service = self.spec.services.get(&service_name).unwrap();
            let image = service.image.clone().unwrap_or_default();
            let container_spec = ContainerSpec {
                image: image.clone(),
                name: Some(generate_name(Some(&self.project_name), &image, &service_name)),
                ports: service.ports.as_ref().map(|p| p.iter().map(|ps| format!("{:?}", ps)).collect()),
                volumes: service.volumes.as_ref().map(|v| v.iter().map(|vs| format!("{:?}", vs)).collect()),
                env: match &service.environment {
                    Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), v.as_ref().map(|val| serde_yaml::to_string(val).unwrap_or_default().trim().to_string()).unwrap_or_default())).collect()),
                    _ => None,
                },
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("com.docker.compose.project".to_string(), self.project_name.clone());
                    labels.insert("com.docker.compose.service".to_string(), service_name.clone());
                    if let Some(ListOrDict::Dict(d)) = &service.labels {
                        for (k, v) in d {
                            labels.insert(k.clone(), v.as_ref().map(|val| serde_yaml::to_string(val).unwrap_or_default().trim().to_string()).unwrap_or_default());
                        }
                    }
                    Some(labels)
                },
                cmd: match &service.command {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| format!("{:?}", v)).collect()),
                    _ => None,
                },
                entrypoint: None,
                network: None, // TODO: set network from service.networks
                rm: Some(false),
                read_only: service.read_only,
            };

            match self.backend.run(&container_spec).await {
                Ok(handle) => {
                    started_containers.push((service_name, handle));
                }
                Err(e) => {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: service_name,
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: rand::random(),
            project_name: self.project_name.clone(),
            services: started_containers.iter().map(|(n, _)| n.clone()).collect(),
        })
    }

    async fn rollback(&self, containers: &[(String, ContainerHandle)], networks: &[String], volumes: &[String]) {
        for (_, handle) in containers.iter().rev() {
            let _ = self.backend.stop(&handle.id, Some(10)).await;
            let _ = self.backend.remove(&handle.id, true).await;
        }
        for net in networks {
            let _ = self.backend.remove_network(net).await;
        }
        for vol in volumes {
            let _ = self.backend.remove_volume(vol).await;
        }
    }

    pub async fn down(&self, services: &[String], _remove_orphans: bool, volumes: bool) -> Result<()> {
        let order = Self::resolve_startup_order(&self.spec)?;
        for service_name in order.iter().rev() {
            if services.is_empty() || services.contains(service_name) {
                // Try to find the container name that would have been generated
                // In a real implementation we would track started container IDs.
                // For now we assume generate_name is the source of truth if we don't have IDs.
                let image = self.spec.services.get(service_name).and_then(|s| s.image.as_deref()).unwrap_or("");
                let container_name = generate_name(Some(&self.project_name), image, service_name);
                let _ = self.backend.remove(&container_name, true).await;
            }
        }
        if services.is_empty() {
            if let Some(networks) = &self.spec.networks {
                for name in networks.keys() {
                    let prefixed_name = format!("{}_{}", self.project_name, name);
                    let _ = self.backend.remove_network(&prefixed_name).await;
                }
            }
            if volumes {
                if let Some(vols) = &self.spec.volumes {
                    for name in vols.keys() {
                        let prefixed_name = format!("{}_{}", self.project_name, name);
                        let _ = self.backend.remove_volume(&prefixed_name).await;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.backend.list(true).await
    }

    pub async fn logs(&self, services: &[String], tail: Option<u32>) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();
        let target_services = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };

        for svc in target_services {
            if let Ok(logs) = self.backend.logs(&svc, tail).await {
                results.insert(svc, format!("{}{}", logs.stdout, logs.stderr));
            }
        }
        Ok(results)
    }

    pub async fn exec(&self, service: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        self.backend.exec(service, cmd, env, workdir).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for svc in target { self.backend.start(&svc).await?; }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for svc in target { self.backend.stop(&svc, None).await?; }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for svc in target {
            self.backend.stop(&svc, None).await?;
            self.backend.start(&svc).await?;
        }
        Ok(())
    }

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }
}
