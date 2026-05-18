## ADDED Requirements

### Requirement: LspClientApi trait definition
The system SHALL define an `LspClientApi` trait with `Send + Sync` bounds that declares the following async methods:
- File lifecycle: `did_open`, `did_change`, `did_close`
- Query methods: `goto_definition`, `references`, `implementations`, `hover`, `diagnostic`, `rename`, `prepare_call_hierarchy`, `incoming_calls`, `outgoing_calls`
- Lifecycle: `shutdown`

The trait SHALL use `#[async_trait]` for dyn-compatible async dispatch.

#### Scenario: Trait is dyn-compatible
- **WHEN** `Box<dyn LspClientApi>` is constructed from a concrete `LspClient`
- **THEN** all trait methods SHALL be callable through the trait object without panics

#### Scenario: Mock implementation is auto-generated
- **WHEN** the trait is annotated with `#[cfg_attr(test, automock)]`
- **THEN** `MockLspClientApi` SHALL be available in test code with `.expect_*()` methods for each trait method

### Requirement: LspClientApi error contract
All trait methods SHALL return `Result<T, ClientError>` using the existing `ClientError` type. This ensures mock expectations can simulate both success and error responses.

#### Scenario: Query method returns error
- **WHEN** a mock is configured to return `Err(ClientError::Transport(...))` for `goto_definition`
- **THEN** the calling handler SHALL receive the error and map it to a tool error response

#### Scenario: Query method returns success
- **WHEN** a mock is configured to return `Ok(vec![location])` for `references`
- **THEN** the calling handler SHALL receive the locations and format them as output
