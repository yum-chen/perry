use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use super::types::ComposeError;
use tokio::process::Command;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
enum VerificationResult {
    Verified(String), // digest
    Failed(String),   // reason
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ComposeError> {
    // 1. Resolve digest (simulation or shell out to backend)
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read = cache.read().unwrap();
        if let Some(res) = read.get(&digest) {
            return match res {
                VerificationResult::Verified(d) => Ok(d.clone()),
                VerificationResult::Failed(r) => Err(ComposeError::VerificationFailed {
                    image: reference.to_string(),
                    reason: r.clone(),
                }),
            };
        }
    }

    // 3. Run cosign verify
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache result
    {
        let mut write = cache.write().unwrap();
        write.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified(d) => Ok(d),
        VerificationResult::Failed(r) => Err(ComposeError::VerificationFailed {
            image: reference.to_string(),
            reason: r,
        }),
    }
}

async fn fetch_image_digest(reference: &str) -> Result<String, ComposeError> {
    if reference.contains('@') {
        return Ok(reference.split('@').last().unwrap().to_string());
    }

    // We need the backend to inspect the image and get its ID/digest.
    // Since verification.rs is in the same crate as mod.rs, we can use the global backend.
    let backend = match super::get_global_backend_instance().await {
        Ok(b) => b,
        Err(e) => return Err(ComposeError::BackendNotAvailable { name: "auto".into(), reason: e }),
    };

    match backend.inspect(reference).await {
        Ok(info) => Ok(info.image),
        Err(_) => {
            // If inspect fails, try listing images to find a match
            let images = backend.list_images().await?;
            for img in images {
                let repo_tag = format!("{}:{}", img.repository, img.tag);
                if repo_tag == reference || img.id.starts_with(reference) {
                    return Ok(img.id);
                }
            }
            Err(ComposeError::NotFound(format!("Image not found: {}", reference)))
        }
    }
}

async fn run_cosign_verify(reference: &str, digest: &str) -> VerificationResult {
    let mut cmd = Command::new("cosign");
    cmd.args([
        "verify",
        "--certificate-identity", CHAINGUARD_IDENTITY,
        "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
        reference
    ]);

    match cmd.output().await {
        Ok(output) if output.status.success() => VerificationResult::Verified(digest.to_string()),
        Ok(output) => VerificationResult::Failed(String::from_utf8_lossy(&output.stderr).to_string()),
        Err(e) => VerificationResult::Failed(e.to_string()),
    }
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git"     => Some("cgr.dev/chainguard/git".to_string()),
        "curl"    => Some("cgr.dev/chainguard/curl".to_string()),
        "wget"    => Some("cgr.dev/chainguard/wget".to_string()),
        "bash"    => Some("cgr.dev/chainguard/bash".to_string()),
        "node"    => Some("cgr.dev/chainguard/node".to_string()),
        "python"  => Some("cgr.dev/chainguard/python".to_string()),
        "go"      => Some("cgr.dev/chainguard/go".to_string()),
        "rust"    => Some("cgr.dev/chainguard/rust".to_string()),
        _         => None,
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}
