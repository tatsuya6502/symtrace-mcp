## 1. Config module

- [ ] 1.1 Add `toml` dependency to `Cargo.toml`
- [ ] 1.2 Create `src/config.rs` with `SymtraceConfig`, `ServerConfig`, and `ProjectEntry` structs (serde Deserialize)
- [ ] 1.3 Implement `SymtraceConfig::load(path)` that reads and parses `.symtrace.toml`
- [ ] 1.4 Implement `SymtraceConfig::implicit(cwd)` that generates the default single-project config
- [ ] 1.5 Add unit tests for config parsing: valid multi-project, valid server-only, invalid TOML, missing file

## 2. ProjectRegistry

- [ ] 2.1 Create `src/project/registry.rs` with `ProjectRegistry` struct containing `Arc<HashMap<PathBuf, Arc<LanguageServerManager>>>` and `sorted_roots: Vec<PathBuf>`
- [ ] 2.2 Implement `ProjectRegistry::new(config, cwd)` that builds managers from config entries, canonicalizes paths, and sorts roots by length descending
- [ ] 2.3 Implement `ProjectRegistry::get_manager_for_file(&self, path: &Path) -> Result<&Arc<LanguageServerManager>>` with longest-prefix-match
- [ ] 2.4 Implement `ProjectRegistry::managers()` iterator for lifecycle operations (idle monitor spawning, shutdown)
- [ ] 2.5 Add unit tests for longest-prefix-match: single match, nested roots, no match

## 3. Module structure

- [ ] 3.1 Create `src/project.rs` module file (not `mod.rs`) with `pub mod registry`
- [ ] 3.2 Wire `config` and `project` modules in `src/main.rs`

## 4. Refactor LanguageServerManager

- [ ] 4.1 Move `IdleMonitor` ownership from `McpServer` into `LanguageServerManager` (each manager creates and owns its own monitor)
- [ ] 4.2 Add `LanguageServerManager::start_idle_monitor()` that spawns the background task and returns `JoinHandle`
- [ ] 4.3 Ensure `LanguageServerManager::new()` accepts `root: PathBuf` and `configs: HashMap<Language, LanguageServerConfig>` (already does — verify)

## 5. Refactor McpServer

- [ ] 5.1 Replace `Arc<LanguageServerManager>` and `Arc<IdleMonitor>` fields with `Arc<ProjectRegistry>`
- [ ] 5.2 Update `McpServer::new()` to build `ProjectRegistry` from config instead of a single manager
- [ ] 5.3 Update `tool_handler!` macro to capture `Arc<ProjectRegistry>` and call `registry.get_manager_for_file()` before delegating to the manager
- [ ] 5.4 Update `McpServer::run()` to spawn idle monitors for all managers via `registry.managers()`
- [ ] 5.5 Update graceful shutdown to iterate all managers in the registry

## 6. Integration

- [ ] 6.1 Update `main()` to attempt `.symtrace.toml` load at CWD, falling back to implicit config
- [ ] 6.2 Verify single-project backward compatibility: no `.symtrace.toml` → identical behavior to pre-change
- [ ] 6.3 Manual test: create a monorepo with two Rust projects and `.symtrace.toml`, verify both get separate rust-analyzer instances via MCP tools
- [ ] 6.4 Verify error message quality: file not in any project, invalid config, missing project root directory
