use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::SymtraceConfig;
use crate::server::manager::{Language, LanguageServerConfig, LanguageServerManager};

#[derive(Debug)]
pub enum RegistryError {
    NoProjectForFile {
        path: PathBuf,
        roots: Vec<PathBuf>,
    },
    Canonicalization {
        root: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::NoProjectForFile { path, roots } => {
                let roots: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
                write!(
                    f,
                    "file {:?} does not belong to any configured project (roots: {})",
                    path,
                    roots.join(", ")
                )
            }
            RegistryError::Canonicalization { root, source } => {
                write!(
                    f,
                    "project root {:?} does not exist or is not accessible: {source}",
                    root
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RegistryError::Canonicalization { source, .. } => Some(source),
            RegistryError::NoProjectForFile { .. } => None,
        }
    }
}

pub struct ProjectRegistry {
    managers: Arc<HashMap<PathBuf, Arc<LanguageServerManager>>>,
    sorted_roots: Vec<PathBuf>,
}

impl ProjectRegistry {
    pub fn new(config: &SymtraceConfig, cwd: &Path) -> Result<Self, RegistryError> {
        let server_configs = Self::build_server_configs(&config.server);
        let project_entries = if config.projects.is_empty() {
            vec![
                cwd.canonicalize()
                    .map_err(|e| RegistryError::Canonicalization {
                        root: cwd.to_path_buf(),
                        source: e,
                    })?,
            ]
        } else {
            config
                .projects
                .iter()
                .map(|p| {
                    let full = cwd.join(&p.root);
                    full.canonicalize()
                        .map_err(|e| RegistryError::Canonicalization {
                            root: p.root.clone(),
                            source: e,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut managers = HashMap::new();
        for root in &project_entries {
            let manager = LanguageServerManager::with_configs(root.clone(), server_configs.clone());
            managers.insert(root.clone(), Arc::new(manager));
        }

        let mut sorted_roots: Vec<PathBuf> = managers.keys().cloned().collect();
        sorted_roots.sort_by_key(|b| std::cmp::Reverse(b.as_os_str().len()));

        Ok(ProjectRegistry {
            managers: Arc::new(managers),
            sorted_roots,
        })
    }

    pub fn get_manager_for_file(
        &self,
        path: &Path,
    ) -> Result<&Arc<LanguageServerManager>, RegistryError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| RegistryError::Canonicalization {
                root: path.to_path_buf(),
                source: e,
            })?;

        for root in &self.sorted_roots {
            if canonical.starts_with(root) {
                return Ok(self
                    .managers
                    .get(root)
                    .expect("sorted_roots derived from managers keys"));
            }
        }

        Err(RegistryError::NoProjectForFile {
            path: path.to_path_buf(),
            roots: self.sorted_roots.clone(),
        })
    }

    pub fn managers(&self) -> impl Iterator<Item = &Arc<LanguageServerManager>> {
        self.managers.values()
    }

    fn build_server_configs(
        server_section: &HashMap<String, crate::config::ServerConfig>,
    ) -> HashMap<Language, LanguageServerConfig> {
        let mut configs = default_server_configs();

        for (lang_name, cfg) in server_section {
            if lang_name == "rust"
                && let Some(rust_cfg) = configs.get_mut(&Language::Rust)
            {
                rust_cfg.command = cfg.command.clone();
                rust_cfg.idle_timeout_secs = cfg.idle_timeout_secs;
            }
        }

        configs
    }
}

fn default_server_configs() -> HashMap<Language, LanguageServerConfig> {
    let mut map = HashMap::new();
    map.insert(
        Language::Rust,
        LanguageServerConfig {
            language: Language::Rust,
            command: "rust-analyzer".to_string(),
            args: vec![],
            extensions: vec!["rs"],
            language_id: "rust",
            idle_timeout_secs: 600,
        },
    );
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectEntry;

    #[test]
    fn single_project_matches_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = SymtraceConfig::implicit(dir.path());
        let registry = ProjectRegistry::new(&config, dir.path()).unwrap();

        let file = dir.path().join("src/main.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::File::create(&file).unwrap();

        let result = registry.get_manager_for_file(&file);
        assert!(result.is_ok());
    }

    #[test]
    fn longest_prefix_match() {
        let base = tempfile::tempdir().unwrap();
        let project_a = base.path().join("project-a");
        let sub_crate = project_a.join("sub-crate");
        std::fs::create_dir_all(&sub_crate).unwrap();

        let config = SymtraceConfig {
            server: HashMap::new(),
            projects: vec![
                ProjectEntry {
                    root: PathBuf::from("project-a"),
                },
                ProjectEntry {
                    root: PathBuf::from("project-a/sub-crate"),
                },
            ],
        };

        let registry = ProjectRegistry::new(&config, base.path()).unwrap();

        let file_in_sub = sub_crate.join("src/lib.rs");
        std::fs::create_dir_all(file_in_sub.parent().unwrap()).unwrap();
        std::fs::File::create(&file_in_sub).unwrap();

        let manager = registry.get_manager_for_file(&file_in_sub).unwrap();

        let file_in_a = project_a.join("src/lib.rs");
        std::fs::create_dir_all(file_in_a.parent().unwrap()).unwrap();
        std::fs::File::create(&file_in_a).unwrap();

        let manager_a = registry.get_manager_for_file(&file_in_a).unwrap();

        assert!(!Arc::ptr_eq(manager, manager_a));
    }

    #[test]
    fn no_match_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let config = SymtraceConfig::implicit(dir.path());
        let registry = ProjectRegistry::new(&config, dir.path()).unwrap();

        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("src/main.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::File::create(&file).unwrap();

        let result = registry.get_manager_for_file(&file);
        assert!(result.is_err());
    }
}
