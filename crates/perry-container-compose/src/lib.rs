//! `perry-container-compose` — Docker Compose-like experience for Apple Container / Podman.
//!
//! Can be used:
//!
//! 1. As a standalone CLI binary (`perry-compose`)
//! 2. As a library imported from Perry TypeScript applications
//! 3. Via FFI from compiled Perry TypeScript code (requires `ffi` feature)

pub mod backend;
pub mod cli;
pub mod commands;
pub mod compose;
pub mod installer;
pub mod config;
pub mod error;
pub mod project;
pub mod orchestrate;
pub mod service;
pub mod types;
pub mod yaml;

pub mod testing;

// FFI exports (Perry TypeScript integration)
#[cfg(feature = "ffi")]
pub mod ffi;

// Re-exports
pub use error::{ComposeError, Result};
pub use types::{ComposeHandle, ComposeService, ComposeSpec};
pub use compose::ComposeEngine;
pub use project::ComposeProject;
pub use backend::{ContainerBackend, CliBackend, CliProtocol, DockerProtocol, AppleContainerProtocol, LimaProtocol, detect_backend};
