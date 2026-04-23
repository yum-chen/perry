use crate::error::Result;
use crate::config::ProjectConfig;
use crate::types::ComposeSpec;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = if let Some(first) = config.files.first() {
            first.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            PathBuf::from(".")
        };

        let env = crate::yaml::load_env(&project_dir, &config.env_files);

        let mut files = config.files.clone();
        if files.is_empty() {
            if project_dir.join("compose.yaml").exists() {
                files.push(project_dir.join("compose.yaml"));
            } else if project_dir.join("docker-compose.yml").exists() {
                files.push(project_dir.join("docker-compose.yml"));
            }
        }

        let spec = crate::yaml::parse_and_merge_files(&files, &env)?;

        let project_name = config.project_name.clone()
            .or_else(|| std::env::var("COMPOSE_PROJECT_NAME").ok())
            .unwrap_or_else(|| {
                project_dir.canonicalize().ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "default".into())
            });

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files: files,
        })
    }

    pub fn engine(&self) -> ComposeEngine {
        let rt = tokio::runtime::Handle::current();
        let backend_arc = rt.block_on(detect_backend()).expect("Failed to detect container backend");

        ComposeEngine::new(self.spec.clone(), backend_arc)
    }
}
