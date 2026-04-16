use crate::error::{ComposeError, Result};
use crate::config::ProjectConfig;
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
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = if let Some(first_file) = config.files.first() {
            first_file.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            PathBuf::from(".")
        };

        // 1. Resolve project name
        let project_name = if let Some(name) = &config.project_name {
            name.clone()
        } else if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
            name
        } else {
            project_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("perry")
                .to_string()
        };

        // 2. Discover compose files if not provided
        let files = if config.files.is_empty() {
            let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
            let mut found = Vec::new();
            for c in candidates {
                let path = project_dir.join(c);
                if path.exists() {
                    found.push(path);
                    break;
                }
            }
            if found.is_empty() {
                return Err(ComposeError::FileNotFound { path: "compose.yaml".to_string() });
            }
            found
        } else {
            config.files.clone()
        };

        // 3. Load environment
        let env = yaml::load_env(&project_dir, &config.env_files);

        // 4. Parse and merge files
        let spec = yaml::parse_and_merge_files(&files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files: files,
        })
    }

    /// Helper for FFI/internal use to load from specific files.
    pub fn load_from_files(files: &[PathBuf], project_name: Option<String>, env_files: &[PathBuf]) -> Result<Self> {
        let config = ProjectConfig::new(files.to_vec(), project_name, env_files.to_vec());
        Self::load(&config)
    }
}
