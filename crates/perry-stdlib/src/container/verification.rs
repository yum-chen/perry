//! Image signature verification using Sigstore/cosign.

use super::types::ContainerLogs;
use perry_container_compose::error::ComposeError;
use std::collections::HashMap;
use std::sync::{RwLock, OnceLock};
use tokio::process::Command;
use std::sync::Arc;
use perry_container_compose::backend::ContainerBackend;

#[derive(Debug, Clone)]
pub enum VerificationResult {
    Verified,
    Failed(String),
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

pub async fn verify_image(reference: &str, backend: &Arc<dyn ContainerBackend>) -> Result<String, ComposeError> {
    let digest = fetch_image_digest(reference, backend).await?;

    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let rd = cache.read().unwrap();
        if let Some(result) = rd.get(&digest) {
            return match result {
                VerificationResult::Verified => Ok(digest),
                VerificationResult::Failed(reason) => Err(ComposeError::VerificationFailed {
                    image: reference.to_string(),
                    reason: reason.clone(),
                }),
            };
        }
    }

    let result = run_cosign_verify(reference).await;

    {
        let mut wr = cache.write().unwrap();
        wr.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified => Ok(digest),
        VerificationResult::Failed(reason) => Err(ComposeError::VerificationFailed {
            image: reference.to_string(),
            reason,
        }),
    }
}

async fn fetch_image_digest(reference: &str, backend: &Arc<dyn ContainerBackend>) -> Result<String, ComposeError> {
    let info = backend.inspect(reference).await?;
    Ok(info.id)
}

async fn run_cosign_verify(reference: &str) -> VerificationResult {
    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity",
            CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer",
            CHAINGUARD_ISSUER,
            "--output",
            "text",
            reference,
        ])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => VerificationResult::Verified,
        Ok(out) => VerificationResult::Failed(String::from_utf8_lossy(&out.stderr).to_string()),
        Err(e) => VerificationResult::Failed(format!("cosign failed: {}", e)),
    }
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git"     => Some("cgr.dev/chainguard/git".to_string()),
        "curl"    => Some("cgr.dev/chainguard/curl".to_string()),
        "wget"    => Some("cgr.dev/chainguard/wget".to_string()),
        "bash"    => Some("cgr.dev/chainguard/bash".to_string()),
        _         => None,
    }
}

pub fn get_default_base_image() -> String {
    "cgr.dev/chainguard/alpine-base".to_string()
}
