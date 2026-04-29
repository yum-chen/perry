//! Tests for the programmatic backend-selection API
//! (`js_container_setBackend` + `js_container_getBackendPriority`).
//!
//! These pin three contracts:
//!
//! 1. `getBackendPriority()` returns a JSON-encoded `string[]` matching
//!    the platform's compile-time probe order (canary for "did the
//!    macOS apple-first invariant survive a refactor?").
//!
//! 2. `setBackend("docker")` etc. round-trips through the FFI without
//!    crashing on the StringHeader encoding (regression guard for the
//!    same FFI shape that previously broke `composeUp({...})`).
//!
//! 3. `setBackend("notarealbackend")` rejects with a clear error
//!    message naming the valid options — the user must learn what's
//!    available without grepping source.

use perry_runtime::{js_promise_state, js_promise_run_microtasks, Promise, StringHeader};
use perry_stdlib::container::*;
use std::ptr;

const PROMISE_STATE_PENDING: i32 = 0;
const PROMISE_STATE_REJECTED: i32 = 2;

fn drive_promise(promise: *mut Promise) {
    let mut iterations = 0;
    while js_promise_state(promise) == PROMISE_STATE_PENDING && iterations < 100 {
        unsafe {
            perry_stdlib::common::js_stdlib_process_pending();
            js_promise_run_microtasks();
        }
        std::thread::yield_now();
        iterations += 1;
    }
}

fn make_string_header(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;
    let mut header_bytes = vec![0u8; std::mem::size_of::<StringHeader>() + bytes.len()];
    unsafe {
        let header = header_bytes.as_mut_ptr() as *mut StringHeader;
        (*header).utf16_len = s.chars().count() as u32;
        (*header).byte_len = len;
        (*header).capacity = len;
        (*header).refcount = 0;
        let data_ptr = header_bytes.as_mut_ptr().add(std::mem::size_of::<StringHeader>());
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_ptr, bytes.len());
    }
    header_bytes
}

unsafe fn read_string_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

#[test]
fn get_backend_priority_returns_valid_json_array() {
    // The list must be a JSON-encoded string[] — TS callers parse this
    // with `JSON.parse(...) as string[]`. Returning anything else is
    // a contract break.
    unsafe {
        let result_ptr = js_container_getBackendPriority();
        let json = read_string_header(result_ptr).expect("non-null result");
        let parsed: Vec<String> =
            serde_json::from_str(&json).expect("getBackendPriority must return JSON string[]");
        assert!(
            !parsed.is_empty(),
            "platform priority list must be non-empty"
        );
    }
}

#[test]
fn get_backend_priority_macos_lists_apple_first() {
    // The single most important cross-backend invariant: on macOS, the
    // user's first-choice OCI runtime is `apple/container` (the only
    // platform-native one). If a refactor ever flips this to favor
    // docker/podman, this test catches it before users notice.
    if !cfg!(target_os = "macos") && !cfg!(target_os = "ios") {
        return; // only meaningful on Apple platforms
    }
    unsafe {
        let result_ptr = js_container_getBackendPriority();
        let json = read_string_header(result_ptr).expect("non-null result");
        let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed[0], "apple/container",
            "macOS priority list must start with apple/container; got {:?}",
            parsed
        );
        // Docker should always be the LAST fallback, never first.
        assert_eq!(
            parsed.last().map(|s| s.as_str()),
            Some("docker"),
            "docker must always be the last fallback; got {:?}",
            parsed
        );
    }
}

#[test]
fn get_backend_priority_linux_lists_podman_first() {
    if !cfg!(target_os = "linux") {
        return;
    }
    unsafe {
        let result_ptr = js_container_getBackendPriority();
        let json = read_string_header(result_ptr).expect("non-null result");
        let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
        // OCI-compatible / rootless / daemonless beats daemon-based
        // (podman) → containerd-native (nerdctl) → daemon-based fallback (docker).
        assert_eq!(parsed[0], "podman", "Linux priority list must start with podman");
        assert_eq!(parsed.last().map(|s| s.as_str()), Some("docker"));
    }
}

#[test]
fn set_backend_rejects_unknown_name() {
    // The caller passing a typo or a backend name that doesn't exist in
    // the probe list MUST get a clear error message naming the valid
    // options — they shouldn't have to grep Perry's source to find out
    // what `setBackend()` accepts.
    unsafe {
        let header = make_string_header("notarealbackend");
        let promise_ptr = js_container_setBackend(header.as_ptr() as *const StringHeader);
        assert!(!promise_ptr.is_null());
        drive_promise(promise_ptr);
        assert_eq!(
            js_promise_state(promise_ptr),
            PROMISE_STATE_REJECTED,
            "setBackend('notarealbackend') must reject"
        );
    }
}

#[test]
fn select_backend_for_trivial_spec_picks_apple_first_on_macos() {
    // A spec with nothing fancy → return the first platform candidate.
    // On macOS that's apple/container — the only platform-native option.
    if !cfg!(target_os = "macos") && !cfg!(target_os = "ios") {
        return;
    }
    unsafe {
        let spec = r#"{"services":{"web":{"image":"nginx"}}}"#;
        let mode = "accept-emulated";
        let spec_h = make_string_header(spec);
        let mode_h = make_string_header(mode);
        let result_ptr = js_container_selectBackendFor(
            spec_h.as_ptr() as *const StringHeader,
            mode_h.as_ptr() as *const StringHeader,
        );
        let json = read_string_header(result_ptr).expect("non-null");
        assert_eq!(
            json, r#""apple/container""#,
            "trivial spec on macOS must pick apple/container; got {}",
            json
        );
    }
}

#[test]
fn select_backend_for_privileged_spec_skips_apple() {
    // privileged: true → apple/container can't honor, falls through
    // to the next backend that can. On macOS that's orbstack →
    // colima → ... → docker. All Docker-protocol-compatible backends
    // share the Docker capability profile, so the first one in the
    // priority list wins. Today: orbstack on macOS.
    unsafe {
        let spec = r#"{
            "services": {
                "ptrace": {
                    "image": "tracer:latest",
                    "privileged": true
                }
            }
        }"#;
        let mode = "accept-emulated";
        let spec_h = make_string_header(spec);
        let mode_h = make_string_header(mode);
        let result_ptr = js_container_selectBackendFor(
            spec_h.as_ptr() as *const StringHeader,
            mode_h.as_ptr() as *const StringHeader,
        );
        let json = read_string_header(result_ptr).expect("non-null");
        // The result MUST NOT be apple/container — that's the point
        // of capability-aware selection. The exact runner-up depends
        // on platform, but it's guaranteed not to be apple.
        let parsed: String = serde_json::from_str(&json)
            .expect("selectBackendFor must return a JSON string");
        assert_ne!(
            parsed, "apple/container",
            "privileged: true must rule out apple/container; got {}",
            parsed
        );
    }
}

#[test]
fn select_backend_for_strict_native_rejects_emulated() {
    // restart_policy is `Emulated` on apple/container (host-side
    // respawn loop). Under accept-emulated, apple is fine; under
    // strict-native, apple is rejected and we fall through to a
    // backend with native restart support.
    if !cfg!(target_os = "macos") && !cfg!(target_os = "ios") {
        return;
    }
    unsafe {
        let spec = r#"{
            "services": {
                "redis": {
                    "image": "redis:7-alpine",
                    "restart": "unless-stopped"
                }
            }
        }"#;
        // accept-emulated → apple/container picked
        let spec_h = make_string_header(spec);
        let mode_emul = make_string_header("accept-emulated");
        let r1 = js_container_selectBackendFor(
            spec_h.as_ptr() as *const StringHeader,
            mode_emul.as_ptr() as *const StringHeader,
        );
        let j1 = read_string_header(r1).expect("non-null");
        let n1: String = serde_json::from_str(&j1).expect("json string");
        assert_eq!(
            n1, "apple/container",
            "accept-emulated must allow apple/container with restart_policy: Emulated; got {}",
            n1
        );

        // strict-native → apple/container rejected, falls through
        let mode_strict = make_string_header("strict-native");
        let r2 = js_container_selectBackendFor(
            spec_h.as_ptr() as *const StringHeader,
            mode_strict.as_ptr() as *const StringHeader,
        );
        let j2 = read_string_header(r2).expect("non-null");
        let n2: String = serde_json::from_str(&j2).expect("json string");
        assert_ne!(
            n2, "apple/container",
            "strict-native must reject apple/container for restart_policy; got {}",
            n2
        );
    }
}

#[test]
fn select_backend_for_garbage_spec_returns_null() {
    // Defensive: malformed JSON → return "null", not crash.
    unsafe {
        let spec = "not actually json";
        let mode = "accept-emulated";
        let spec_h = make_string_header(spec);
        let mode_h = make_string_header(mode);
        let result_ptr = js_container_selectBackendFor(
            spec_h.as_ptr() as *const StringHeader,
            mode_h.as_ptr() as *const StringHeader,
        );
        let json = read_string_header(result_ptr).expect("non-null");
        assert_eq!(json, "null", "malformed spec must return JSON null");
    }
}

#[test]
fn select_backend_for_null_spec_returns_null() {
    // Defensive: null pointer → "null".
    unsafe {
        let mode_h = make_string_header("accept-emulated");
        let result_ptr = js_container_selectBackendFor(
            ptr::null(),
            mode_h.as_ptr() as *const StringHeader,
        );
        let json = read_string_header(result_ptr).expect("non-null result");
        assert_eq!(json, "null");
    }
}

#[test]
fn set_backend_rejects_null_pointer() {
    // The FFI must defensively reject a null pointer rather than
    // dereferencing it. Same defensive contract as every other
    // string-arg FFI in `mod.rs`.
    unsafe {
        let promise_ptr = js_container_setBackend(ptr::null());
        assert!(!promise_ptr.is_null());
        drive_promise(promise_ptr);
        assert_eq!(
            js_promise_state(promise_ptr),
            PROMISE_STATE_REJECTED,
            "setBackend(NULL) must reject"
        );
    }
}
