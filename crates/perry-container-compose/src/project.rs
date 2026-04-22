use crate::error::{ComposeError, Result};
use crate::config::{ProjectConfig, resolve_compose_files, resolve_project_name};
use crate::types::ComposeSpec;
use crate::yaml::{parse_and_merge_files, load_env};
use std::path::{Path, PathBuf};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = if let Some(first) = config.compose_files.first() {
            first.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            std::env::current_dir().map_err(ComposeError::IoError)?
        };

        let project_name = resolve_project_name(config.project_name.as_deref(), &project_dir);
        let compose_files = resolve_compose_files(&config.compose_files)?;
        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }

    pub fn load_from_files(files: &[PathBuf], project_name: Option<&str>, env_files: &[PathBuf]) -> Result<Self> {
        let config = ProjectConfig {
            compose_files: files.to_vec(),
            project_name: project_name.map(|s| s.to_string()),
            env_files: env_files.to_vec(),
        };
        Self::load(&config)
    }
}
