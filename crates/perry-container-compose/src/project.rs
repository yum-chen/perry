use crate::error::{ComposeError, Result};
use crate::config::{ProjectConfig, resolve_compose_files, resolve_project_name};
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_compose_yaml};
use std::path::PathBuf;
use std::fs;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let files = resolve_compose_files(&config.compose_files)?;
        let first_file = files.first().ok_or_else(|| ComposeError::FileNotFound { path: "no compose files found".into() })?;
        let project_dir = first_file.parent().unwrap_or(&std::path::Path::new(".")).to_path_buf();
        let project_name = resolve_project_name(config.project_name.as_deref(), &project_dir);

        // Load environment variables for interpolation
        let env = load_env(&project_dir, &config.env_files);

        let mut merged_spec = ComposeSpec::default();
        for file in &files {
            let content = fs::read_to_string(file).map_err(ComposeError::IoError)?;
            let spec = parse_compose_yaml(&content, &env)?;
            merged_spec.merge(spec);
        }

        Ok(Self {
            spec: merged_spec,
            project_name,
            project_dir,
            compose_files: files,
        })
    }
}
