//! `perry-container-compose` — Docker Compose-like experience for Apple Container / Podman.
//!
//! Can be used:
//!
//! 1. As a standalone CLI binary (`perry-compose`)
//! 2. As a library imported from Perry TypeScript applications
//! 3. Via FFI from compiled Perry TypeScript code (requires `ffi` feature)

pub mod types;
pub mod error;
pub mod yaml;
pub mod project;
pub mod service;
pub mod compose;
pub mod backend;
pub mod cli;
pub mod config;
pub mod workload;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use error::{ComposeError, Result};
pub use types::{ComposeSpec, ComposeService, ComposeHandle};
pub use compose::ComposeEngine;
pub use project::ComposeProject;
pub use backend::{ContainerBackend, OciBackend, BackendDriver, OciCommandBuilder, BackendProbeResult, detect_backend};

// External crate re-exports for integration tests
pub use indexmap;
