# Spec Delta

## ADDED Requirements

### Requirement: Call hierarchy client capability
For every supported language, the `initialize` request SHALL declare the `textDocument.callHierarchy` client capability with `dynamicRegistration` set to `false`. Some language servers advertise `callHierarchyProvider` only when the client declares this capability, so the call hierarchy tools depend on it.

#### Scenario: TypeScript initialization declares call hierarchy
- **WHEN** a TypeScript language server is started
- **THEN** the `initialize` request capabilities SHALL contain `textDocument.callHierarchy` with `dynamicRegistration: false`

#### Scenario: Rust initialization declares call hierarchy
- **WHEN** a Rust language server is started
- **THEN** the `initialize` request capabilities SHALL contain `textDocument.callHierarchy` with `dynamicRegistration: false`

#### Scenario: Call hierarchy tools work once the server advertises the provider
- **WHEN** a TypeScript language server that gates `callHierarchyProvider` on this client capability (typescript-language-server 3.0.0 or later with TypeScript 3.8 or later) is initialized with these capabilities
- **THEN** the system SHALL treat call hierarchy as supported and `incoming_calls` / `outgoing_calls` SHALL NOT return the "does not support call hierarchy" error
