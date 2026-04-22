use crate::error::{ComposeError, Result};
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_and_merge_files};
use std::path::{Path, PathBuf};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let files = resolve_compose_files(&config.files);
        if files.is_empty() {
            return Err(ComposeError::FileNotFound { path: "compose.yaml".into() });
        }

        let primary_file = &files[0];
        let project_dir = primary_file.parent().unwrap_or(Path::new(".")).to_path_buf();
        let project_name = resolve_project_name(config.project_name.as_deref(), &project_dir);

        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files: files,
        })
    }
}
