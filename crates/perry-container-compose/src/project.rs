use crate::error::Result;
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
    /// Load a compose project by following the resolution chain (req 9.1–9.8).
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let files = crate::config::resolve_compose_files(&config.compose_files)?;

        // Use directory of the primary compose file as project_dir (req 9.7).
        let primary_file = &files[0];
        let project_dir = primary_file.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let project_name = crate::config::resolve_project_name(
            config.project_name.as_deref(),
            &project_dir
        );

        // Load environment variables (req 7.8, 7.9).
        let env = crate::yaml::load_env(&project_dir, &config.env_files);

        // Parse and merge files (req 7.10, 9.2).
        let spec = crate::yaml::parse_and_merge_files(&files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files: files,
        })
    }
}
