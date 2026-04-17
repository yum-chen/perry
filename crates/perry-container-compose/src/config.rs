use std::path::{Path, PathBuf};

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

pub fn resolve_project_name(project_dir: &Path, explicit_name: Option<String>) -> String {
    if let Some(name) = explicit_name {
        return name;
    }
    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
        .to_string()
}

pub fn resolve_compose_files(project_dir: &Path, explicit_files: Vec<PathBuf>) -> Vec<PathBuf> {
    if !explicit_files.is_empty() {
        return explicit_files;
    }
    if let Ok(files) = std::env::var("COMPOSE_FILE") {
        return files.split(':').map(PathBuf::from).collect();
    }
    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let p = project_dir.join(c);
        if p.exists() {
            return vec![p];
        }
    }
    vec![]
}
