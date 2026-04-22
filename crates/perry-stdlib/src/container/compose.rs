use indexmap::IndexMap;
use crate::container::types::*;
use crate::container::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
pub use perry_container_compose::error::{ComposeError, Result};

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
            for (name, config_opt) in networks {
                let config = config_opt.as_ref();
                let net_config = NetworkConfig {
                    driver: config.and_then(|c| c.driver.clone()),
                    labels: match config.and_then(|c| c.labels.as_ref()) {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), v.as_ref().and_then(|val| val.as_str()).unwrap_or("").to_string())).collect(),
                        _ => HashMap::new(),
                    },
                    internal: config.and_then(|c| c.internal).unwrap_or(false),
                    enable_ipv6: config.and_then(|c| c.enable_ipv6).unwrap_or(false),
                };
                if let Err(e) = self.backend.create_network(name, &net_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(name.clone());
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (name, config_opt) in volumes {
                let config = config_opt.as_ref();
                let vol_config = VolumeConfig {
                    driver: config.and_then(|c| c.driver.clone()),
                    labels: match config.and_then(|c| c.labels.as_ref()) {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), v.as_ref().and_then(|val| val.as_str()).unwrap_or("").to_string())).collect(),
                        _ => HashMap::new(),
                    },
                };
                if let Err(e) = self.backend.create_volume(name, &vol_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_volumes.push(name.clone());
            }
        }

        for service_name in order {
            let service = self.spec.services.get(&service_name).unwrap();
            let image = service.image.clone().unwrap_or_default();

            if let Err(e) = self.backend.pull_image(&image).await {
                 self.rollback(&started_containers, &created_networks, &created_volumes).await;
                 return Err(ComposeError::ImagePullFailed {
                     service: service_name,
                     image: image.clone(),
                     message: e.to_string(),
                 });
            }

            let container_spec = ContainerSpec {
                image: image.clone(),
                name: Some(service_name.clone()),
                ports: service.ports.as_ref().map(|p| p.iter().map(|ps| format!("{:?}", ps)).collect()),
                volumes: service.volumes.as_ref().map(|v| v.iter().map(|vs| format!("{:?}", vs)).collect()),
                env: match &service.environment {
                    Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), v.as_ref().and_then(|val| val.as_str()).unwrap_or("").to_string())).collect()),
                    _ => None,
                },
                cmd: match &service.command {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()),
                    _ => None,
                },
                entrypoint: None,
                network: None,
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

    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        let order = Self::resolve_startup_order(&self.spec)?;
        for service_name in order.iter().rev() {
             let _ = self.backend.remove(service_name, true).await;
        }
        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let _ = self.backend.remove_network(name).await;
            }
        }
        if volumes {
            if let Some(vols) = &self.spec.volumes {
                for name in vols.keys() {
                    let _ = self.backend.remove_volume(name).await;
                }
            }
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.backend.list(true).await
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        if let Some(svc) = service {
             self.backend.logs(svc, tail).await
        } else {
             Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
        }
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        self.backend.exec(service, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        for svc in services { self.backend.start(svc).await?; }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        for svc in services { self.backend.stop(svc, None).await?; }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        for svc in services {
            self.backend.stop(svc, None).await?;
            self.backend.start(svc).await?;
        }
        Ok(())
    }
}
