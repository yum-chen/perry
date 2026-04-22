use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::process::Stdio;
use tokio::process::Command;
use crate::container::types::ContainerError;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> String {
    match tool {
        "node" => "cgr.dev/chainguard/node".to_string(),
        "python" => "cgr.dev/chainguard/python".to_string(),
        "go" => "cgr.dev/chainguard/go".to_string(),
        "rust" => "cgr.dev/chainguard/rust".to_string(),
        _ => get_default_base_image().to_string(),
    }
}

#[derive(Debug, Clone)]
enum VerificationResult {
    Verified(String), // digest
    Failed(String),   // reason
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

/// Verify an OCI image reference using Sigstore/cosign keyless verification.
pub async fn verify_image(reference: &str) -> Result<String, String> {
    // 1. Check cache first
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read = cache.read().unwrap();
        if let Some(res) = read.get(reference) {
            return match res {
                VerificationResult::Verified(d) => Ok(d.clone()),
                VerificationResult::Failed(r) => Err(ContainerError::VerificationFailed {
                    image: reference.to_string(),
                    reason: r.clone(),
                }.to_string()),
            };
        }
    }

    // 2. Perform verification via cosign
    let result = match perform_cosign_verify(reference).await {
        Ok(digest) => VerificationResult::Verified(digest),
        Err(reason) => VerificationResult::Failed(reason),
    };

    // 3. Update cache
    {
        let mut write = cache.write().unwrap();
        write.insert(reference.to_string(), result.clone());
    }

    match result {
        VerificationResult::Verified(d) => Ok(d),
        VerificationResult::Failed(r) => Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason: r,
        }.to_string()),
    }
}

async fn perform_cosign_verify(reference: &str) -> Result<String, String> {
    // Check if cosign is available
    let cosign_bin = match which::which("cosign") {
        Ok(path) => path,
        Err(_) => return Err("cosign binary not found on PATH".to_string()),
    };

    // Execute cosign verify
    // We use keyless verification against Chainguard identities
    let output = Command::new(cosign_bin)
        .arg("verify")
        .arg("--certificate-identity").arg(CHAINGUARD_IDENTITY)
        .arg("--certificate-oidc-issuer").arg(CHAINGUARD_ISSUER)
        .arg(reference)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to execute cosign: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    // Parse digest from output (usually the first field of the JSON output)
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(arr) = json.as_array() {
            if let Some(first) = arr.first() {
                if let Some(critical) = first.get("critical") {
                    if let Some(identity) = critical.get("image") {
                        if let Some(digest) = identity.get("docker-manifest-digest") {
                            return Ok(digest.as_str().unwrap_or_default().to_string());
                        }
                    }
                }
            }
        }
    }

    // Fallback if JSON parsing fails but exit code was 0
    Ok("verified".to_string())
}
