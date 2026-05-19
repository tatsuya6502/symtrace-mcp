## MODIFIED Requirements

### Requirement: Language server lifecycle management
`LanguageServerManager` SHALL manage language server lifecycle. `ServerEntry` SHALL hold the client as `Box<dyn LspClientApi>` instead of `LspClient`, enabling trait-object dispatch for both production clients and test mocks.

#### Scenario: Server startup boxes the client
- **WHEN** a language server is started via `start_server_internal`
- **THEN** the system SHALL create `LspClient::start()`, wait for indexing, and store it as `Box<dyn LspClientApi>` in `ServerEntry`

#### Scenario: Server shutdown via trait object
- **WHEN** `stop_server` removes a `ServerEntry`
- **THEN** the system SHALL call `FileManager::close_all` with the trait object, then call `shutdown` on the trait object
