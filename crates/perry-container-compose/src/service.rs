use crate::error::Result;
use md5::{Digest, Md5};

/// Generate a unique container name: {service_name}-{md5_image_hash}-{random_hex}
pub fn generate_name(image: &str, service_name: &str) -> String {
    // MD5 hash of the image name for a stable prefix
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness across multiple instances
    let random_suffix: u32 = rand::random();

    // Sanitize service name
    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

/// Legacy helper - redirects to new name generation
pub fn service_container_name(service: &crate::types::ComposeService, service_name: &str) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }
    generate_name(service.image.as_deref().unwrap_or("unknown"), service_name)
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
