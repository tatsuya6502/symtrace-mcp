# Spec Delta

## ADDED Requirements

### Requirement: Optional initialization options
`LspClient::start` SHALL accept optional initialization options from its caller. When options are provided, the `initialize` request SHALL carry them unchanged as `initializationOptions`. When no options are provided, the `initialize` request SHALL omit the `initializationOptions` field entirely rather than sending `null`.

#### Scenario: Options provided
- **WHEN** `LspClient::start` is called with initialization options
- **THEN** the `initialize` request SHALL include an `initializationOptions` field equal to the provided value

#### Scenario: No options provided
- **WHEN** `LspClient::start` is called without initialization options
- **THEN** the `initialize` request SHALL NOT include an `initializationOptions` field
