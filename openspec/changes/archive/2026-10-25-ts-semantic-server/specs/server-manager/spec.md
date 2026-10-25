# Spec Delta

## ADDED Requirements

### Requirement: TypeScript semantic-server-only mode
The `initialize` request sent to a TypeScript language server SHALL include `initializationOptions` with `tsserver.useSyntaxServer` set to `"never"`, so that every request is answered by the semantic tsserver with whole-project knowledge, including requests that arrive while the project is still loading. The `initialize` request sent to a Rust language server SHALL NOT contain an `initializationOptions` field.

#### Scenario: TypeScript initialization disables the syntax server
- **WHEN** a TypeScript language server is started
- **THEN** the `initialize` request SHALL contain `initializationOptions` equal to `{"tsserver": {"useSyntaxServer": "never"}}`

#### Scenario: Rust initialization sends no initialization options
- **WHEN** a Rust language server is started
- **THEN** the `initialize` request SHALL NOT contain an `initializationOptions` field (neither an object nor `null`)

#### Scenario: First TypeScript query resolves across files
- **WHEN** the first tool call after a TypeScript language server starts is `goto_definition` on a call to a function imported from another file
- **THEN** the response SHALL point to the function's definition in the other file, not to the import statement in the queried file
