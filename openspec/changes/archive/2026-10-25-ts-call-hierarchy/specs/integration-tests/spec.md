# Spec Delta

## MODIFIED Requirements

### Requirement: Integration tests cover all MCP tools
Integration tests for each language SHALL test all 8 tools: `find_references`, `goto_definition`, `find_implementations`, `incoming_calls`, `outgoing_calls`, `hover`, `diagnostics`, and `rename`.

#### Scenario: tools/list returns all registered tools
- **WHEN** a `tools/list` request is sent
- **THEN** the response SHALL include all 8 tools with valid JSON schemas

#### Scenario: goto_definition resolves to correct location
- **WHEN** `goto_definition` is called on a function call site
- **THEN** the response SHALL contain a location pointing to the function definition

#### Scenario: hover returns type information
- **WHEN** `hover` is called on a typed symbol
- **THEN** the response SHALL contain hover content

#### Scenario: diagnostics returns successfully
- **WHEN** `diagnostics` is called on a file with no errors
- **THEN** the response SHALL indicate success (may return empty diagnostics)

#### Scenario: incoming_calls returns callers
- **WHEN** `incoming_calls` is called with `json: true` on a function that the fixture calls, for either language
- **THEN** the response SHALL succeed and contain at least one caller

#### Scenario: outgoing_calls returns callees
- **WHEN** `outgoing_calls` is called with `json: true` on a fixture function that calls another function, for either language
- **THEN** the response SHALL succeed and contain at least one callee

#### Scenario: TypeScript call hierarchy crosses file boundaries
- **WHEN** `incoming_calls` is called on a function exported from one TypeScript fixture file and called from another fixture file that imports it
- **THEN** the response SHALL contain a caller located in the importing file
