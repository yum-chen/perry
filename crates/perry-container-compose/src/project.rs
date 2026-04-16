use crate::error::{ComposeError, Result};
use crate::config::ProjectConfig;
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = config.files.first()
            .and_then(|f| f.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let project_name = if let Some(name) = &config.project_name {
            name.clone()
        } else if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
            name
        } else {
            project_dir
                .canonicalize()
                .unwrap_or_else(|_| project_dir.clone())
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("perry-project")
                .to_string()
        };

        let compose_files = if config.files.is_empty() {
            let default_files = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
            let mut found = Vec::new();
            for f in default_files {
                let p = project_dir.join(f);
                if p.exists() {
                    found.push(p);
                    break;
                }
            }
            if found.is_empty() {
                 return Err(ComposeError::FileNotFound { path: "compose.yaml".into() });
            }
            found
        } else {
            config.files.clone()
        };

        let env = yaml::load_env(&project_dir, &config.env_files);
        let spec = yaml::parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectConfig;
    use std::fs;

    #[test]
    fn test_project_load_basic() {
        let temp_dir = std::env::temp_dir().join(format!("perry-test-{}", rand::random::<u32>()));
        fs::create_dir_all(&temp_dir).unwrap();
        let compose_file = temp_dir.join("compose.yaml");
        fs::write(&compose_file, "services:\n  web:\n    image: nginx").unwrap();

        let config = ProjectConfig::new(vec![compose_file.clone()], Some("my-proj".into()), vec![]);
        let project = ComposeProject::load(&config).unwrap();

        assert_eq!(project.project_name, "my-proj");
        assert!(project.spec.services.contains_key("web"));

        fs::remove_dir_all(temp_dir).unwrap();
    }
}
