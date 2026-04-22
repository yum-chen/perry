use md5::{Md5, Digest};
use hex;
use rand;

/// Generate a unique container name: {service_name}_{md5_prefix}_{random_hex}.
pub fn service_container_name(svc: &crate::types::ComposeService, service_name: &str) -> String {
    if let Some(name) = &svc.container_name {
        return name.clone();
    }
    let mut hasher = Md5::new();
    hasher.update(svc.image_ref(service_name).as_bytes());
    let result = hasher.finalize();
    let hash = hex::encode(&result)[0..8].to_string();
    let rand_suffix: u32 = rand::random();
    format!("{}_{}_{:08x}", service_name, hash, rand_suffix)
}
