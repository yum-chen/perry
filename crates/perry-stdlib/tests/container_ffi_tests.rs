//! FFI contract tests for perry/container and perry/compose.
//!
//! These tests verify that FFI functions handle null pointers and malformed
//! JSON correctly by returning a valid promise that eventually rejects.

use perry_runtime::{js_promise_state, js_promise_run_microtasks, Promise, StringHeader};
use perry_stdlib::container::*;
use std::ptr;

const PROMISE_STATE_PENDING: i32 = 0;
const PROMISE_STATE_FULFILLED: i32 = 1;
const PROMISE_STATE_REJECTED: i32 = 2;

/// Helper to create a fake StringHeader on the stack for testing.
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

/// Drive the promise to completion by running microtasks and processing pending stdlib ops.
fn drive_promise(promise: *mut Promise) {
    // In a real environment, the tokio runtime would run the spawned task.
    // Here we need to ensure the task has a chance to run.
    // Since we are testing early validation errors, they often happen before spawning
    // or the spawned task finishes immediately.

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

// ============ js_container_run ============

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_js_container_run_null() {
    unsafe {
        let p = js_container_run(ptr::null());
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_js_container_run_malformed() {
    let header = make_string_header("{invalid json}");
    unsafe {
        let p = js_container_run(header.as_ptr() as *const StringHeader);
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// ============ js_container_composeUp ============

// Feature: perry-container | Layer: ffi-contract | Req: 6.1 | Property: -
#[test]
fn test_js_container_composeUp_null() {
    unsafe {
        let p = js_container_composeUp(ptr::null());
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.1 | Property: -
#[test]
fn test_js_container_composeUp_malformed() {
    let header = make_string_header("not a json object");
    unsafe {
        let p = js_container_composeUp(header.as_ptr() as *const StringHeader);
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// ============ js_compose_ps ============

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_js_compose_ps_not_found() {
    unsafe {
        // Stack ID 99999 should not exist
        let p = js_compose_ps(99999.0);
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// ============ js_container_inspect ============

// Feature: perry-container | Layer: ffi-contract | Req: 3.1 | Property: -
#[test]
fn test_js_container_inspect_null() {
    unsafe {
        let p = js_container_inspect(ptr::null());
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 11.1        | test_js_container_run_null | ffi-contract |
| 11.1        | test_js_container_run_malformed | ffi-contract |
| 6.1         | test_js_container_composeUp_null | ffi-contract |
| 6.1         | test_js_container_composeUp_malformed | ffi-contract |
| 6.6         | test_js_compose_ps_not_found | ffi-contract |
| 3.1         | test_js_container_inspect_null | ffi-contract |

Deferred Requirements:
- none
*/
