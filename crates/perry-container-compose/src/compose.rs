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
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self { spec, backend }
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

    pub async fn up(&self) -> Result<ComposeHandle> {
        let order = Self::resolve_startup_order(&self.spec)?;
        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers: Vec<(String, ContainerHandle)> = Vec::new();

        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let config_raw = config.clone().unwrap_or_default();
                let network_config = NetworkConfig {
                    driver: config_raw.driver.clone(),
                    labels: match &config_raw.labels {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), v.as_ref().map(|val| format!("{:?}", val)).unwrap_or_default())).collect(),
                        _ => HashMap::new(),
                    },
                    internal: config_raw.internal.unwrap_or(false),
                    enable_ipv6: config_raw.enable_ipv6.unwrap_or(false),
                };

                if let Err(e) = self.backend.create_network(name, &network_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(name.clone());
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let config_raw = config.clone().unwrap_or_default();
                let volume_config = VolumeConfig {
                    driver: config_raw.driver.clone(),
                    labels: match &config_raw.labels {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), v.as_ref().map(|val| format!("{:?}", val)).unwrap_or_default())).collect(),
                        _ => HashMap::new(),
                    },
                };
                if let Err(e) = self.backend.create_volume(name, &volume_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_volumes.push(name.clone());
            }
        }

        for service_name in order {
            let service = self.spec.services.get(&service_name).unwrap();
            let image = service.image.clone().unwrap_or_default();
            let container_spec = ContainerSpec {
                image: image.clone(),
                name: Some(generate_name(&image, &service_name)),
                ports: service.ports.as_ref().map(|p| p.iter().map(|ps| format!("{:?}", ps)).collect()),
                volumes: service.volumes.as_ref().map(|v| v.iter().map(|vs| format!("{:?}", vs)).collect()),
                env: match &service.environment {
                    Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), v.as_ref().map(|val| format!("{:?}", val)).unwrap_or_default())).collect()),
                    _ => None,
                },
                cmd: match &service.command {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| format!("{:?}", v)).collect()),
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
            project_name: self.spec.name.clone().unwrap_or_else(|| "default".into()),
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

    pub async fn down(&self, volumes: bool) -> Result<()> {
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
