use md5::{Digest, Md5};
use crate::backend::ContainerBackend;

pub fn generate_name(project_name: Option<&str>, image: &str, service_name: &str) -> String {
    // MD5 hash of the image name for a stable prefix
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness across multiple instances of the same image
    let random_suffix: u32 = rand::random();

    // Sanitize service name: replace non-alphanumeric (except hyphen) with underscore
    let safe_service_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    if let Some(project) = project_name {
        let safe_project_name: String = project
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();
        format!("{}-{}-{}-{:08x}", safe_project_name, safe_service_name, short_hash, random_suffix)
    } else {
        format!("{}-{}-{:08x}", safe_service_name, short_hash, random_suffix)
    }
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}

pub async fn exists(backend: &dyn ContainerBackend, name: &str) -> bool {
    backend.inspect(name).await.is_ok()
}

pub async fn is_running(backend: &dyn ContainerBackend, name: &str) -> bool {
    if let Ok(info) = backend.inspect(name).await {
        info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_name_format() {
        let name = generate_name(Some("myproject"), "nginx", "web");
        assert!(name.starts_with("myproject-web-"));
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[2].len(), 8);
        assert_eq!(parts[3].len(), 8);
    }

    #[test]
    fn test_generate_name_deterministic_prefix() {
        let name1 = generate_name(None, "nginx", "web");
        let name2 = generate_name(None, "nginx", "api");
        let parts1: Vec<&str> = name1.split('-').collect();
        let parts2: Vec<&str> = name2.split('-').collect();
        assert_eq!(parts1[1], parts2[1]);
    }
}
