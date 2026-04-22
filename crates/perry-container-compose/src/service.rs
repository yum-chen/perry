use md5::{Digest, Md5};

pub fn generate_name(project_name: &str, service_name: &str, service: &crate::types::ComposeService) -> crate::error::Result<String> {
    if let Some(name) = service.container_name.as_ref() {
        return Ok(name.clone());
    }

    // Serialize the entire service config to YAML for a stable, config-based hash.
    let yaml = serde_yaml::to_string(service)
        .map_err(|e| crate::error::ComposeError::ParseError(e))?;

    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);

    // Use the first 8 chars of the hash as a stable, human-readable suffix
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness across multiple instances
    let random_suffix: u32 = rand::random();

    // Use project_name and service_name as prefix for better identification
    let safe_service_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    Ok(format!("{}-{}-{}-{:08x}", project_name, safe_service_name, short_hash, random_suffix))
}

pub fn needs_build(service: &crate::types::ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
