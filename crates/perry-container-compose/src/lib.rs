pub mod types;
pub mod error;
pub mod yaml;
pub mod project;
pub mod service;
pub mod compose;
pub mod backend;
pub mod cli;
pub mod config;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use error::{ComposeError, Result, BackendProbeResult};
pub use types::{ComposeSpec, ComposeService, ComposeHandle};
pub use compose::ComposeEngine;
pub use project::ComposeProject;
pub use backend::{ContainerBackend, detect_backend, NetworkConfig, VolumeConfig, DockerProtocol, AppleContainerProtocol, LimaProtocol, DockerBackend, AppleBackend, LimaBackend, CliBackend};
