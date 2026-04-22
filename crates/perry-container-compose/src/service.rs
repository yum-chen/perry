use crate::error::{Result, ComposeError};
use md5::{Digest, Md5};
use crate::types::ComposeService;

pub fn generate_name(service: &ComposeService) -> Result<String> {
    let yaml = serde_yaml::to_string(service)
        .map_err(ComposeError::ParseError)?;

    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);

    let short_hash = &hash_str[..8];
    let random_suffix: u32 = rand::random();
    Ok(format!("{}-{:08x}", short_hash, random_suffix))
}

pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
