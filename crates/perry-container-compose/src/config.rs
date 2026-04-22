//! Project configuration and environment variable resolution.

use crate::error::{ComposeError, Result};
use std::path::{Path, PathBuf};

/// Default compose file names to search for (in priority order)
pub const DEFAULT_COMPOSE_FILES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// Project-level configuration.
pub struct ProjectConfig {
    /// Compose file paths
    pub compose_files: Vec<PathBuf>,
    /// Project name (from -p flag or COMPOSE_PROJECT_NAME or directory name)
    pub project_name: Option<String>,
    /// Extra environment file paths (from --env-file flags)
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    /// Create a new project config from CLI options.
    pub fn new(
        compose_files: Vec<PathBuf>,
        project_name: Option<String>,
        env_files: Vec<PathBuf>,
    ) -> Self {
        ProjectConfig {
            compose_files,
            project_name,
            env_files,
        }
    }
}

/// Resolve project name.
///
/// Priority: CLI `-p` flag > `COMPOSE_PROJECT_NAME` env var > directory name
pub fn resolve_project_name(
    cli_name: Option<&str>,
    project_dir: &Path,
) -> String {
    if let Some(name) = cli_name {
        return name.to_string();
    }

    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }

    project_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Resolve compose file paths.
///
/// Priority: CLI `-f` flags > `COMPOSE_FILE` env var (pathsep-separated) > default file search
pub fn resolve_compose_files(cli_files: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if !cli_files.is_empty() {
        return Ok(cli_files.to_vec());
    }

    if let Ok(compose_file_env) = std::env::var("COMPOSE_FILE") {
        #[cfg(target_os = "windows")]
        let separator = ";";
        #[cfg(not(target_os = "windows"))]
        let separator = ":";

        let files: Vec<PathBuf> = compose_file_env
            .split(separator)
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();

        if !files.is_empty() {
            return Ok(files);
        }
    }

    let cwd = std::env::current_dir()?;
    find_default_compose_file(&cwd)
}

/// Find the default compose file in a directory.
pub fn find_default_compose_file(dir: &Path) -> Result<Vec<PathBuf>> {
    for name in DEFAULT_COMPOSE_FILES {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Ok(vec![candidate]);
        }
    }
    Err(ComposeError::FileNotFound {
        path: format!(
            "No compose file found in {} (tried: {})",
            dir.display(),
            DEFAULT_COMPOSE_FILES.join(", ")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_project_name_cli_priority() {
        let tmp = std::env::temp_dir().join("perry-test-project");
        std::fs::create_dir_all(&tmp).ok();

        let name = resolve_project_name(Some("my-project"), &tmp);
        assert_eq!(name, "my-project");
    }

    #[test]
    fn test_resolve_project_name_dir_fallback() {
        let tmp = std::env::temp_dir().join("perry-test-project-2");
        std::fs::create_dir_all(&tmp).ok();

        let name = resolve_project_name(None, &tmp);
        assert_eq!(name, "perry-test-project-2");
    }
}
