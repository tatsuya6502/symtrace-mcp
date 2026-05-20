## ADDED Requirements

### Requirement: ProjectRegistry routes files to project managers
The system SHALL provide a `ProjectRegistry` that maps file paths to the appropriate `LanguageServerManager` using longest-prefix-match on configured project roots.

#### Scenario: File under a configured project
- **WHEN** a tool call targets a file whose path is under a configured project root
- **THEN** the registry SHALL return the `LanguageServerManager` for that project

#### Scenario: File under nested projects (longest prefix)
- **WHEN** a file path matches multiple project roots (e.g., `project-a/` and `project-a/sub-crate/`)
- **THEN** the registry SHALL route to the project with the longest matching prefix

#### Scenario: File not under any project
- **WHEN** a tool call targets a file that does not fall under any configured project root
- **THEN** the registry SHALL return an error indicating the file does not belong to a configured project

### Requirement: ProjectRegistry is immutable after construction
The project-to-manager mapping SHALL be built once at startup from configuration and SHALL NOT be modified at runtime.

#### Scenario: Runtime lookup
- **WHEN** a tool handler requests a manager for a file path
- **THEN** the registry performs a read-only lookup without any locks or synchronization overhead beyond `Arc` reference counting

### Requirement: ProjectRegistry initializes one manager per project
Each configured project root SHALL have its own `LanguageServerManager` instance with its own `root` path, server configs, server state, and environment variables.

#### Scenario: Multi-project initialization
- **WHEN** `.symtrace.toml` configures two projects (`project-a` and `project-b`)
- **THEN** the registry SHALL create two independent `LanguageServerManager` instances, each rooted at their respective project directory

#### Scenario: Project with env vars initializes manager with env
- **WHEN** `.symtrace.toml` configures a project with `env = { DATABASE_URL = "postgres://..." }`
- **THEN** the registry SHALL pass the environment variables to the `LanguageServerManager` for that project

#### Scenario: Project without env vars
- **WHEN** `.symtrace.toml` configures a project without an `env` field
- **THEN** the registry SHALL create a manager with no additional environment variables

### Requirement: ProjectRegistry provides iteration for lifecycle management
The registry SHALL provide a way to iterate over all project managers to support operations like spawning idle monitors and graceful shutdown.

#### Scenario: Spawning idle monitors at startup
- **WHEN** `McpServer::run()` starts
- **THEN** it iterates all managers in the registry and spawns an idle monitor task for each

#### Scenario: Graceful shutdown
- **WHEN** the MCP server shuts down
- **THEN** the system iterates all managers and shuts down their running language servers
