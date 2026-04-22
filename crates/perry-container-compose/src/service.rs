use md5::{Digest, Md5};

/// Generate a unique container name following SPEC.md 4.8.
/// service::generate_name(image, service_name) -> MD5(image)[0..8] + random u32 suffix.
pub fn generate_name(image: &str, service_name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);

    // Use the first 8 chars of the hash
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness
    let random_suffix: u32 = rand::random();

    format!("{}-{}-{:08x}", service_name, short_hash, random_suffix)
}

pub fn needs_build(service: &crate::types::ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
