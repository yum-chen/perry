//! Image verification for Perry.

use std::collections::HashMap;
use tokio::sync::Mutex;
use std::sync::OnceLock;
use crate::container::types::ContainerError;

static VERIFICATION_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, String> {
    let cache = VERIFICATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().await;

    if let Some(digest) = cache.get(reference) {
        return Ok(digest.clone());
    }

    // Stub: real implementation would use cosign
    let digest = format!("sha256:{}", reference.len());
    cache.insert(reference.to_string(), digest.clone());
    Ok(digest)
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> String {
    format!("cgr.dev/chainguard/{}", tool)
}

impl From<String> for ContainerError {
    fn from(s: String) -> Self {
        ContainerError::VerificationFailed { image: "unknown".into(), reason: s }
    }
}
