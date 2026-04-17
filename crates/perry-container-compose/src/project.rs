use crate::error::{ComposeError, Result};
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_and_merge_files};
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let compose_files = resolve_compose_files(&project_dir, config.files.clone());

        if compose_files.is_empty() {
            return Err(ComposeError::FileNotFound { path: "No compose file found".into() });
        }

        let project_name = resolve_project_name(&project_dir, config.project_name.clone());
        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }
}
