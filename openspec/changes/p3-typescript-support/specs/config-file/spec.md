## MODIFIED Requirements

### Requirement: Configuration loaded from .symtrace.toml
The system SHALL read configuration from a `.symtrace.toml` file located in the current working directory (CWD) at startup. The `[server]` section SHALL accept arbitrary language keys including `"rust"` and `"typescript"`. Each key maps to a `ServerConfig` with `command` and optional `idle_timeout_secs`.

#### Scenario: Config file exists and is valid
- **WHEN** `.symtrace.toml` exists in CWD and contains valid TOML
- **THEN** the system SHALL parse it and create project and server configurations

#### Scenario: Config file does not exist
- **WHEN** `.symtrace.toml` does not exist in CWD
- **THEN** the system SHALL use an implicit single-project configuration rooted at CWD with default server settings (Rust and TypeScript defaults)

#### Scenario: Config file has parse errors
- **WHEN** `.symtrace.toml` exists but contains invalid TOML or violates the expected schema
- **THEN** the system SHALL return a descriptive error and refuse to start

#### Scenario: TypeScript server configured
- **WHEN** `.symtrace.toml` contains `[server.typescript]` with a `command`
- **THEN** the system SHALL create a TypeScript server config overriding the default `typescript-language-server` command

#### Scenario: Both Rust and TypeScript configured
- **WHEN** `.symtrace.toml` contains both `[server.rust]` and `[server.typescript]`
- **THEN** the system SHALL create configs for both languages, and files will be routed by extension
