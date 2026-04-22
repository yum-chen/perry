//! YAML parsing, environment variable interpolation, `.env` loading,
//! and multi-file merge.

use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============ Environment variable interpolation ============

/// Expand `${VAR}`, `${VAR:-default}`, `${VAR:+value}`, and `$VAR` in a string.
pub fn interpolate(input: &str, env: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            match chars.peek() {
                Some('{') => {
                    chars.next(); // consume '{'
                    let expr = read_until_close(&mut chars);
                    let expanded = expand_expr(&expr, env);
                    result.push_str(&expanded);
                }
                Some('$') => {
                    chars.next();
                    result.push('$');
                }
                Some(&c) if c.is_alphanumeric() || c == '_' => {
                    let name = read_plain_var(&mut chars, c);
                    let val = lookup(&name, env);
                    result.push_str(&val);
                }
                _ => {
                    result.push('$');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn read_until_close(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut expr = String::new();
    let mut depth = 1usize;
    for ch in chars.by_ref() {
        match ch {
            '{' => {
                depth += 1;
                expr.push(ch);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                expr.push(ch);
            }
            _ => expr.push(ch),
        }
    }
    expr
}

fn read_plain_var(chars: &mut std::iter::Peekable<std::str::Chars>, first: char) -> String {
    let mut name = String::new();
    name.push(first);
    chars.next();
    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' {
            name.push(c);
            chars.next();
        } else {
            break;
        }
    }
    name
}

fn expand_expr(expr: &str, env: &HashMap<String, String>) -> String {
    // ${VAR:-default}
    if let Some(pos) = expr.find(":-") {
        let name = &expr[..pos];
        let default = &expr[pos + 2..];
        let val = lookup(name, env);
        if val.is_empty() {
            return default.to_owned();
        }
        return val;
    }

    // ${VAR:+value}
    if let Some(pos) = expr.find(":+") {
        let name = &expr[..pos];
        let value = &expr[pos + 2..];
        let val = lookup(name, env);
        if !val.is_empty() {
            return value.to_owned();
        }
        return String::new();
    }

    lookup(expr, env)
}

fn lookup(name: &str, env: &HashMap<String, String>) -> String {
    if let Some(v) = env.get(name) {
        return v.clone();
    }
    std::env::var(name).unwrap_or_default()
}

// ============ .env file loading ============

/// Parse a `.env` file into a key→value map.
///
/// Rules:
/// - Lines starting with `#` are comments
/// - Empty lines are skipped
/// - Format: `KEY=VALUE` or `KEY="VALUE"` or `KEY='VALUE'`
/// - Inline `#` comments after unquoted values are stripped
pub fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, raw_val)) = line.split_once('=') {
            let key = key.trim().to_owned();
            let val = parse_value(raw_val.trim());
            map.insert(key, val);
        }
    }

    map
}

fn parse_value(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }

    // Double-quoted
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        let inner = &raw[1..raw.len() - 1];
        return inner.replace("\\n", "\n").replace("\\\"", "\"");
    }

    // Single-quoted
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return raw[1..raw.len() - 1].to_owned();
    }

    // Strip inline comment
    if let Some(pos) = raw.find(" #") {
        raw[..pos].trim().to_owned()
    } else {
        raw.to_owned()
    }
}

/// Load environment from .env files.
///
/// Process environment takes precedence over .env files.
/// Explicit `--env-file` files override default .env.
pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    // Default .env in project directory
    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            for (k, v) in parse_dotenv(&content) {
                env.entry(k).or_insert(v);
            }
        }
    }

    // Explicit --env-file flags
    for ef in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(ef) {
            for (k, v) in parse_dotenv(&content) {
                env.insert(k, v);
            }
        }
    }

    env
}

// ============ YAML parsing ============

/// Parse a compose YAML string into a `ComposeSpec` after interpolation.
pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate(yaml, env);
    ComposeSpec::parse_str(&interpolated)
}

// ============ Multi-file merge ============

/// Parse and merge multiple compose files in order.
///
/// Later files override earlier ones (last-writer-wins).
/// Returns `ComposeError::FileNotFound` if any file is missing.
pub fn parse_and_merge_files(
    files: &[PathBuf],
    env: &HashMap<String, String>,
) -> Result<ComposeSpec> {
    let mut merged: Option<ComposeSpec> = None;

    for file_path in files {
        let content = std::fs::read_to_string(file_path).map_err(|_| ComposeError::FileNotFound {
            path: file_path.display().to_string(),
        })?;

        let spec = parse_compose_yaml(&content, env)?;

        match &mut merged {
            None => merged = Some(spec),
            Some(base) => base.merge(spec),
        }
    }

    Ok(merged.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dotenv_basic() {
        let content = "FOO=bar\nBAZ=qux\n# comment\n\nEMPTY=";
        let map = parse_dotenv(content);
        assert_eq!(map["FOO"], "bar");
        assert_eq!(map["BAZ"], "qux");
        assert_eq!(map["EMPTY"], "");
    }

    #[test]
    fn test_parse_dotenv_quoted() {
        let content = r#"A="hello world"
B='single quoted'
C="with \"escape\""
"#;
        let map = parse_dotenv(content);
        assert_eq!(map["A"], "hello world");
        assert_eq!(map["B"], "single quoted");
        assert_eq!(map["C"], "with \"escape\"");
    }

    #[test]
    fn test_interpolate_simple() {
        let mut env = HashMap::new();
        env.insert("NAME".into(), "world".into());
        assert_eq!(interpolate("Hello ${NAME}!", &env), "Hello world!");
    }

    #[test]
    fn test_interpolate_default() {
        let env = HashMap::new();
        assert_eq!(interpolate("${MISSING:-fallback}", &env), "fallback");
    }

    #[test]
    fn test_interpolate_conditional() {
        let mut env = HashMap::new();
        env.insert("SET".into(), "yes".into());
        assert_eq!(interpolate("${SET:+value}", &env), "value");
        let empty: HashMap<String, String> = HashMap::new();
        assert_eq!(interpolate("${UNSET:+value}", &empty), "");
    }

    #[test]
    fn test_interpolate_dollar_dollar() {
        let env = HashMap::new();
        assert_eq!(interpolate("$$FOO", &env), "$FOO");
    }

    #[test]
    fn test_parse_compose_yaml() {
        let yaml = r#"
services:
  web:
    image: nginx
"#;
        let env = HashMap::new();
        let spec = parse_compose_yaml(yaml, &env).unwrap();
        assert!(spec.services.contains_key("web"));
        assert_eq!(spec.services["web"].image.as_deref(), Some("nginx"));
    }

    #[test]
    fn test_interpolate_in_yaml() {
        let yaml = r#"
services:
  web:
    image: ${IMAGE:-nginx}
"#;
        let mut env = HashMap::new();
        env.insert("IMAGE".into(), "redis".into());
        let spec = parse_compose_yaml(yaml, &env).unwrap();
        assert_eq!(spec.services["web"].image.as_deref(), Some("redis"));

        // Default fallback
        let empty_env = HashMap::new();
        let spec2 = parse_compose_yaml(yaml, &empty_env).unwrap();
        assert_eq!(spec2.services["web"].image.as_deref(), Some("nginx"));
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
