## MODIFIED Requirements

### Requirement: Configuration loaded from .symtrace.toml
The system SHALL read configuration from a `.symtrace.toml` file located in the current working directory (CWD) at startup. The `[server]` section SHALL accept arbitrary language keys including `"rust"` and `"typescript"`. Each key maps to a `ServerConfig` with `command`, optional `idle_timeout_secs` and optional `index_timeout_secs`. Each `[[projects]]` entry SHALL accept an optional `env` field as an inline table mapping environment variable names to values. The optional `[logging]` section SHALL accept a `level` field with values: `off`, `error`, `warn`, `info`, `debug`, `trace`, or `tracing-subscriber::filter::Targets` filter syntax. The default value for `level` SHALL be `info`.

#### Scenario: Config file exists and is valid
- **WHEN** `.symtrace.toml` exists in CWD and contains valid TOML
- **THEN** the system SHALL parse it and create project and server configurations

#### Scenario: Config file does not exist
- **WHEN** `.symtrace.toml` does not exist in CWD
- **THEN** the system SHALL use an implicit single-project configuration rooted at CWD with default server settings (Rust and TypeScript defaults) and default logging settings

#### Scenario: Config file has parse errors
- **WHEN** `.symtrace.toml` exists but contains invalid TOML or violates the expected schema
- **THEN** the system SHALL return a descriptive error and refuse to start

#### Scenario: TypeScript server configured
- **WHEN** `.symtrace.toml` contains `[server.typescript]` with a `command`
- **THEN** the system SHALL create a TypeScript server config overriding the default `typescript-language-server` command

#### Scenario: Both Rust and TypeScript configured
- **WHEN** `.symtrace.toml` contains both `[server.rust]` and `[server.typescript]`
- **THEN** the system SHALL create configs for both languages, and files will be routed by extension

#### Scenario: Project with env vars
- **WHEN** `.symtrace.toml` contains a `[[projects]]` entry with `env = { DATABASE_URL = "postgres://..." }`
- **THEN** the system SHALL parse the env vars and associate them with that project entry

#### Scenario: Project without env vars
- **WHEN** `.symtrace.toml` contains a `[[projects]]` entry without an `env` field
- **THEN** the system SHALL treat the project as having no additional environment variables

#### Scenario: Logging section with level
- **WHEN** `.symtrace.toml` contains `[logging] level = "debug"`
- **THEN** the system SHALL use `debug` as the default log level (overridable by `SYMTRACE_LOG` env var)

#### Scenario: Logging section omitted
- **WHEN** `.symtrace.toml` does not contain a `[logging]` section
- **THEN** the system SHALL use `info` as the default log level

#### Scenario: Logging section with no level
- **WHEN** `.symtrace.toml` contains `[logging]` with no `level` field
- **THEN** the system SHALL use `info` as the default log level

#### Scenario: Server section with index timeout
- **WHEN** `.symtrace.toml` contains `[server.rust]` with `command = "rust-analyzer"` and `index_timeout_secs = 180`
- **THEN** the system SHALL parse `index_timeout_secs` as 180 for the Rust server config

#### Scenario: Invalid index timeout value
- **WHEN** `index_timeout_secs` is negative, not an integer, or greater than 86400 (one day)
- **THEN** the system SHALL return a descriptive error and refuse to start

### Requirement: Server configuration with global defaults
The `[server.<language>]` section SHALL define language server command, idle timeout and index wait timeout. These apply to all projects unless overridden. The index wait timeout (`index_timeout_secs`) SHALL default to 30 seconds when omitted.

#### Scenario: Server command specified
- **WHEN** the config contains `[server.rust] command = "rust-analyzer"`
- **THEN** all Rust projects SHALL use `rust-analyzer` as their server command

#### Scenario: Server section absent
- **WHEN** the config has no `[server]` section
- **THEN** the system SHALL use hardcoded defaults (rust-analyzer with 600s idle timeout and 30s index timeout)

#### Scenario: Index timeout specified
- **WHEN** the config contains `[server.rust]` with `index_timeout_secs = 180`
- **THEN** all Rust projects SHALL wait up to 180 seconds for rust-analyzer to become ready

#### Scenario: Index timeout omitted
- **WHEN** the config contains `[server.rust]` with a `command` but no `index_timeout_secs`
- **THEN** all Rust projects SHALL wait up to 30 seconds for rust-analyzer to become ready
