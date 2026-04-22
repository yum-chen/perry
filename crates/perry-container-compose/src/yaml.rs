use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use regex::Regex;

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a YAML string.
pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"(\$\$|\$\{([a-zA-Z0-9_]*)(?::-([^}]*))?\}|\$([a-zA-Z0-9_]+))").unwrap();

    re.replace_all(yaml, |caps: &regex::Captures| {
        if &caps[0] == "$$" {
            return "$".to_string();
        }

        let var_name = caps.get(2).map(|m| m.as_str())
            .or_else(|| caps.get(4).map(|m| m.as_str()))
            .unwrap_or("");

        if var_name.is_empty() {
             if let Some(default) = caps.get(3) {
                return default.as_str().to_string();
             }
             return caps[0].to_string();
        }

        match env.get(var_name) {
            Some(val) if !val.is_empty() => val.clone(),
            _ => {
                if let Some(default) = caps.get(3) {
                    default.as_str().to_string()
                } else {
                    "".to_string()
                }
            }
        }
    }).to_string()
}

/// Load a .env file and merge with process environment.
pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    // Default .env
    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((k, v)) = line.split_once('=') {
                    env.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
                }
            }
        }
    }

    // Extra env files
    for ef in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(ef) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((k, v)) = line.split_once('=') {
                    env.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }

    env
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    ComposeSpec::parse_str(&interpolated)
}

pub fn interpolate(yaml: &str, env: &HashMap<String, String>) -> String {
    interpolate_yaml(yaml, env)
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let mut root_spec = ComposeSpec::default();

    for file in files {
        let content = std::fs::read_to_string(file)
            .map_err(|_| ComposeError::FileNotFound { path: file.to_string_lossy().to_string() })?;
        let spec = parse_compose_yaml(&content, env)?;

        if root_spec.services.is_empty() {
            root_spec = spec;
        } else {
            root_spec.merge(spec);
        }
    }

    Ok(root_spec)
}

#[cfg(test)]
mod tests_v5 {
    use super::*;
    use proptest::prelude::*;

    // Feature: alloy-container, Property 6: YAML round-trip (CLI path)
    proptest! {
        #[test]
        fn test_yaml_roundtrip(name in ".*", version in ".*") {
            let spec = ComposeSpec {
                name: Some(name),
                version: Some(version),
                ..Default::default()
            };
            let yaml_str = spec.to_yaml().unwrap();
            let de = ComposeSpec::parse_str(&yaml_str).unwrap();
            assert_eq!(spec.name, de.name);
            assert_eq!(spec.version, de.version);
        }
    }
}

#[cfg(test)]
mod tests_v5 {
    use super::*;
    use proptest::prelude::*;

    // Feature: alloy-container, Property 6: YAML round-trip (CLI path)
    proptest! {
        #[test]
        fn test_yaml_roundtrip(name in ".*", version in ".*") {
            let spec = ComposeSpec {
                name: Some(name),
                version: Some(version),
                ..Default::default()
            };
            let yaml_str = spec.to_yaml().unwrap();
            let de = ComposeSpec::parse_str(&yaml_str).unwrap();
            assert_eq!(spec.name, de.name);
            assert_eq!(spec.version, de.version);
        }
    }
}
