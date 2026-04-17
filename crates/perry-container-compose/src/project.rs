use crate::error::{ComposeError, Result};
use crate::config::{self, ProjectConfig};
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::{Path, PathBuf};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    /// Load a project by resolving names, files, and merging YAML.
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let files = config::resolve_compose_files(config.files.clone());
        if files.is_empty() {
            return Err(ComposeError::FileNotFound { path: "no compose files found".to_string() });
        }

        let first_file = &files[0];
        let project_dir = first_file.parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        let abs_project_dir = std::fs::canonicalize(&project_dir)
            .unwrap_or_else(|_| project_dir.clone());

        let project_name = config::resolve_project_name(
            config.project_name.clone(),
            &abs_project_dir
        );

        let env = yaml::load_env(&abs_project_dir, &config.env_files);
        let spec = yaml::parse_and_merge_files(&files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir: abs_project_dir,
            compose_files: files,
        })
    }
}
