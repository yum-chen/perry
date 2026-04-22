//! `perry-container-compose` — Docker Compose-like experience for Apple Container / Podman.

pub mod backend;
pub mod cli;
pub mod compose;
pub mod config;
pub mod error;
pub mod project;
pub mod service;
pub mod types;
pub mod yaml;

// FFI exports (Perry TypeScript integration)
#[cfg(feature = "ffi")]
pub mod ffi;

// Re-exports
pub use error::{ComposeError, Result};
pub use types::{ComposeHandle, ComposeService, ComposeSpec};
pub use compose::{ComposeEngine, resolve_startup_order};
pub use project::ComposeProject;
pub use backend::{ContainerBackend, CliBackend, CliProtocol, DockerProtocol, AppleContainerProtocol, LimaProtocol, BackendProbeResult, detect_backend};
pub use indexmap;
