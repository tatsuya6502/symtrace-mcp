use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SymtraceConfig {
    #[serde(default)]
    pub server: HashMap<String, ServerConfig>,
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub command: String,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

fn default_idle_timeout() -> u64 {
    600
}

#[derive(Debug, Deserialize)]
pub struct ProjectEntry {
    pub root: PathBuf,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

use std::collections::HashMap;

#[derive(Debug)]
pub enum ConfigError {
    Parse(toml::de::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Parse(e) => write!(f, "config parse error: {e}"),
            ConfigError::Io(e) => write!(f, "config I/O error: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Parse(e) => Some(e),
            ConfigError::Io(e) => Some(e),
        }
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::Parse(e)
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl SymtraceConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: SymtraceConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn implicit(cwd: &Path) -> Self {
        SymtraceConfig {
            server: HashMap::new(),
            projects: vec![ProjectEntry {
                root: cwd.to_path_buf(),
                env: HashMap::new(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_multi_project() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".symtrace.toml");
        let mut f = std::fs::File::create(&config_path).unwrap();
        write!(
            f,
            r#"
[server.rust]
command = "rust-analyzer"

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
"#
        )
        .unwrap();

        let config = SymtraceConfig::load(&config_path).unwrap();
        assert_eq!(config.projects.len(), 2);
        assert_eq!(config.projects[0].root, PathBuf::from("project-a"));
        assert_eq!(config.projects[1].root, PathBuf::from("project-b"));
        assert_eq!(config.server["rust"].command, "rust-analyzer");
    }

    #[test]
    fn load_server_only() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".symtrace.toml");
        let mut f = std::fs::File::create(&config_path).unwrap();
        write!(
            f,
            r#"
[server.rust]
command = "/custom/rust-analyzer"
"#
        )
        .unwrap();

        let config = SymtraceConfig::load(&config_path).unwrap();
        assert!(config.projects.is_empty());
        assert_eq!(config.server["rust"].command, "/custom/rust-analyzer");
    }

    #[test]
    fn load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".symtrace.toml");
        std::fs::write(&config_path, "this is [not valid {{{{").unwrap();

        let result = SymtraceConfig::load(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn implicit_generates_single_project() {
        let cwd = Path::new("/tmp/test-workspace");
        let config = SymtraceConfig::implicit(cwd);
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].root, cwd);
        assert!(config.server.is_empty());
    }

    #[test]
    fn load_typescript_server() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".symtrace.toml");
        let mut f = std::fs::File::create(&config_path).unwrap();
        write!(
            f,
            r#"
[server.rust]
command = "rust-analyzer"

[server.typescript]
command = "typescript-language-server"

[[projects]]
root = "my-project"
"#
        )
        .unwrap();

        let config = SymtraceConfig::load(&config_path).unwrap();
        assert_eq!(config.server["rust"].command, "rust-analyzer");
        assert_eq!(
            config.server["typescript"].command,
            "typescript-language-server"
        );
    }

    #[test]
    fn load_project_with_env() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".symtrace.toml");
        let mut f = std::fs::File::create(&config_path).unwrap();
        write!(
            f,
            r#"
[[projects]]
root = "my-app"
env = {{ DATABASE_URL = "postgres://localhost/mydb", RUST_LOG = "debug" }}
"#
        )
        .unwrap();

        let config = SymtraceConfig::load(&config_path).unwrap();
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].root, PathBuf::from("my-app"));
        assert_eq!(
            config.projects[0].env.get("DATABASE_URL").unwrap(),
            "postgres://localhost/mydb"
        );
        assert_eq!(config.projects[0].env.get("RUST_LOG").unwrap(), "debug");
    }

    #[test]
    fn load_project_without_env_backward_compat() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".symtrace.toml");
        let mut f = std::fs::File::create(&config_path).unwrap();
        write!(
            f,
            r#"
[[projects]]
root = "legacy-project"
"#
        )
        .unwrap();

        let config = SymtraceConfig::load(&config_path).unwrap();
        assert_eq!(config.projects.len(), 1);
        assert!(config.projects[0].env.is_empty());
    }
}
