//! Unit tests for image verification and Chainguard lookup.

use perry_stdlib::container::verification::*;

// Feature: perry-container | Layer: unit | Req: 15.5 | Property: -
#[test]
fn test_chainguard_image_lookup() {
    assert_eq!(get_chainguard_image("git"), Some("cgr.dev/chainguard/git".to_string()));
    assert_eq!(get_chainguard_image("node"), Some("cgr.dev/chainguard/node".to_string()));
    assert_eq!(get_chainguard_image("rust"), Some("cgr.dev/chainguard/rust".to_string()));
    assert_eq!(get_chainguard_image("nonexistent"), None);
}

// Feature: perry-container | Layer: unit | Req: 15.5 | Property: -
#[test]
fn test_default_base_image() {
    assert_eq!(get_default_base_image(), "cgr.dev/chainguard/alpine-base");
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 15.5        | test_chainguard_image_lookup | unit |
| 15.5        | test_default_base_image | unit |

Deferred Requirements:
- Req 15.1-15.4: Requires live network and 'cosign' binary for Sigstore verification.
- Req 15.7: Verification cache idempotence requires actual verification runs.
*/
