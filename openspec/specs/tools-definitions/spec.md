## ADDED Requirements

### Requirement: Handler query dispatch is testable with mocks
Handler functions (`find_references`, `goto_definition`, `find_implementations`, `hover`, `diagnostics`, `rename`, `incoming_calls`, `outgoing_calls`) SHALL dispatch LSP queries through `dyn LspClientApi`. This enables unit tests that inject `MockLspClientApi` without spawning a real language server.

#### Scenario: Handler calls trait method for references
- **WHEN** `find_references` is invoked with a valid file path and position
- **THEN** the handler SHALL call `LspClientApi::references` on the trait object obtained from `ServerEntry`

#### Scenario: Handler calls trait method for hover
- **WHEN** `hover` is invoked with a valid file path and position
- **THEN** the handler SHALL call `LspClientApi::hover` on the trait object

#### Scenario: Handler receives error from mock
- **WHEN** a mock client returns `ClientError::Transport("connection lost")` for `goto_definition`
- **THEN** the handler SHALL return a tool error response with the error message

#### Scenario: FileManager ensure_open via trait object
- **WHEN** a handler calls `entry.file_manager.ensure_open(&mut entry.client, ...)`
- **THEN** `FileManager` SHALL call `did_open` or `did_change` on the `dyn LspClientApi` trait object
