## MODIFIED Requirements

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
