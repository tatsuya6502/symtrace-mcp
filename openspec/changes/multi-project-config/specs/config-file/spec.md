## ADDED Requirements

### Requirement: Configuration loaded from .symtrace.toml
The system SHALL read configuration from a `.symtrace.toml` file located in the current working directory (CWD) at startup.

#### Scenario: Config file exists and is valid
- **WHEN** `.symtrace.toml` exists in CWD and contains valid TOML
- **THEN** the system SHALL parse it and create project and server configurations

#### Scenario: Config file does not exist
- **WHEN** `.symtrace.toml` does not exist in CWD
- **THEN** the system SHALL use an implicit single-project configuration rooted at CWD with default server settings

#### Scenario: Config file has parse errors
- **WHEN** `.symtrace.toml` exists but contains invalid TOML or violates the expected schema
- **THEN** the system SHALL log an error and exit with a non-zero status code

### Requirement: Project roots specified in config
The `[[projects]]` section SHALL define a list of project root directories, each specified as a path relative to CWD.

#### Scenario: Multiple projects configured
- **WHEN** the config contains two `[[projects]]` entries with `root = "project-a"` and `root = "project-b"`
- **THEN** the system SHALL create two project entries with canonicalized absolute paths

#### Scenario: Projects section absent
- **WHEN** the config has a `[server.rust]` section but no `[[projects]]`
- **THEN** the system SHALL use an implicit single project at CWD with the specified server configuration

### Requirement: Server configuration with global defaults
The `[server.<language>]` section SHALL define language server command and idle timeout. These apply to all projects unless overridden.

#### Scenario: Server command specified
- **WHEN** the config contains `[server.rust] command = "rust-analyzer"`
- **THEN** all Rust projects SHALL use `rust-analyzer` as their server command

#### Scenario: Server section absent
- **WHEN** the config has no `[server]` section
- **THEN** the system SHALL use hardcoded defaults (rust-analyzer with 600s idle timeout)

### Requirement: Project root paths are canonicalized
All project root paths from the config SHALL be canonicalized (resolved to absolute paths with symlinks followed) before being used for routing.

#### Scenario: Relative path in config
- **WHEN** a project root is specified as `"project-a"` (relative)
- **THEN** the system SHALL resolve it to an absolute canonical path relative to CWD

#### Scenario: Symlink in project root
- **WHEN** a project root path contains a symlink
- **THEN** the system SHALL resolve the symlink to its real path for consistent prefix matching
