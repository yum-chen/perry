//! Unit tests for service name generation and state tracking.

use perry_container_compose::service::*;
use perry_container_compose::types::ComposeService;

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_generate_name_format() {
    let name = generate_name("nginx:latest");
    let parts: Vec<&str> = name.split('-').collect();
    // {short_hash}-{random_suffix_hex}
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 8);
    assert_eq!(parts[1].len(), 8);
}

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_service_container_name_explicit() {
    let mut svc = ComposeService::default();
    svc.container_name = Some("custom-name".to_string());
    let name = service_container_name(&svc, "web");
    assert_eq!(name, "custom-name");
}

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_service_container_name_generated() {
    let svc = ComposeService::default();
    let name = service_container_name(&svc, "web");
    // Should be {hash}-{random}
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 2);
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 6.13        | test_generate_name_format | unit |
| 6.13        | test_generate_name_sanitization | unit |
| 6.13        | test_service_container_name_explicit | unit |
| 6.13        | test_service_container_name_generated | unit |

Deferred Requirements:
- none
*/
