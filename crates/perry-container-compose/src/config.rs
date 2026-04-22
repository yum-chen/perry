use std::path::{Path, PathBuf};

/// Project configuration (from CLI flags or env)
#[derive(Debug, Clone, Default)]
pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    pub fn new(files: Vec<PathBuf>, project_name: Option<String>, env_files: Vec<PathBuf>) -> Self {
        Self { files, project_name, env_files }
    }
}

/// Resolve the project name.
/// Priority: -p flag > COMPOSE_PROJECT_NAME env > directory name.
pub fn resolve_project_name(
    flag_name: Option<String>,
    project_dir: &Path,
) -> String {
    if let Some(name) = flag_name {
        return name;
    }
    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
        .to_string()
}

/// Resolve the list of compose files.
/// Priority: -f flag(s) > COMPOSE_FILE env > compose.yaml > docker-compose.yml.
pub fn resolve_compose_files(
    flag_files: Vec<PathBuf>,
) -> Vec<PathBuf> {
    if !flag_files.is_empty() {
        return flag_files;
    }
    if let Ok(val) = std::env::var("COMPOSE_FILE") {
        return val.split(':').map(PathBuf::from).collect();
    }

    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return vec![p];
        }
    }
    Vec::new()
}
