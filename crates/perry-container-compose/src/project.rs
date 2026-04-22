use crate::error::Result;
use crate::config::ProjectConfig;
use crate::types::ComposeSpec;
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        // TODO: Implement full project loading
        Ok(Self {
            spec: ComposeSpec::default(),
            project_name: config.project_name.clone().unwrap_or_default(),
            project_dir: PathBuf::from("."),
            compose_files: config.files.clone(),
        })
    }
}
