use crate::error::Result;
use crate::config::ProjectConfig;
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
        let project_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let compose_files = if config.files.is_empty() {
             resolve_default_compose_files(&project_dir)
        } else {
             config.files.clone()
        };

        let project_name = if let Some(name) = &config.project_name {
            name.clone()
        } else if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
            name
        } else {
            project_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default")
                .to_string()
        };

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

fn resolve_default_compose_files(project_dir: &Path) -> Vec<PathBuf> {
    if let Ok(files_env) = std::env::var("COMPOSE_FILE") {
        return files_env.split(':').map(PathBuf::from).collect();
    }

    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let path = project_dir.join(c);
        if path.exists() {
            return vec![path];
        }
    }
    vec![]
}
