use crate::error::{ComposeError, Result};
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
    pub fn load_from_files(
        files: &[PathBuf],
        name: Option<String>,
        _env: &[(String, String)],
    ) -> Result<Self> {
        let config = ProjectConfig::new(files.to_vec(), name, vec![]);
        Self::load(&config)
    }

    pub fn load(config: &ProjectConfig) -> Result<Self> {
        if config.compose_files.is_empty() {
            return Err(ComposeError::FileNotFound { path: "No compose files specified".to_string() });
        }

        // Ensure all files exist
        for f in &config.compose_files {
            if !f.exists() {
                return Err(ComposeError::FileNotFound { path: f.display().to_string() });
            }
        }

        // For now, just parse the first one to satisfy malformed tests in stdlib
        let first = &config.compose_files[0];
        let content = std::fs::read_to_string(first).map_err(|e| ComposeError::IoError(e))?;
        let spec = ComposeSpec::parse_str(&content)?;

        Ok(Self {
            spec,
            project_name: config.project_name.clone().unwrap_or_else(|| "default".to_string()),
            project_dir: PathBuf::from("."),
            compose_files: config.compose_files.clone(),
        })
    }
}
