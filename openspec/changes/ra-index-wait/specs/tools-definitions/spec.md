## ADDED Requirements

### Requirement: Empty result while the language server is indexing
When a position query (`find_references`, `goto_definition`, `find_implementations`, `hover`, `incoming_calls`, `outgoing_calls`, `rename`) produces an empty result and the language server was indexing when the query was sent or when the result arrived, the tool SHALL return an MCP error with code `-32603` and a message starting with `language server is still indexing`, instead of its "no results" output.

#### Scenario: Empty location list while indexing
- **WHEN** `find_references`, `goto_definition` or `find_implementations` gets an empty location list while the language server is indexing
- **THEN** the system SHALL return an MCP error with code `-32603` and a message stating that the language server is still indexing and the call should be retried in a few seconds

#### Scenario: No hover information while indexing
- **WHEN** `hover` gets no hover information while the language server is indexing
- **THEN** the system SHALL return the indexing error instead of "No hover information available"

#### Scenario: No callable symbol while indexing
- **WHEN** `incoming_calls` or `outgoing_calls` cannot resolve a callable symbol at the position while the language server is indexing
- **THEN** the system SHALL return the indexing error instead of "No callable symbol at this position"

#### Scenario: No callers or callees while indexing
- **WHEN** `incoming_calls` or `outgoing_calls` resolves a symbol but gets no calls while the language server is indexing
- **THEN** the system SHALL return the indexing error instead of "No callers found" or "No callees found"

#### Scenario: Nothing to rename while indexing
- **WHEN** `rename` gets no workspace edit while the language server is indexing
- **THEN** the system SHALL return the indexing error instead of "No rename changes"

#### Scenario: Empty diagnostics while indexing
- **WHEN** `diagnostics` gets an empty list while the language server is indexing
- **THEN** the system SHALL return its normal "No diagnostics found" output, because an empty report is a valid answer

#### Scenario: Server becomes indexing during the query
- **WHEN** the language server was ready when the query was sent but reports that it is indexing before the empty result arrives
- **THEN** the system SHALL return the indexing error

#### Scenario: Non-empty result while indexing
- **WHEN** a position query gets a non-empty result while the language server is indexing
- **THEN** the system SHALL return the result in the normal output format

#### Scenario: Server that does not report status
- **WHEN** a position query gets an empty result from a language server that has never reported its status
- **THEN** the system SHALL return the tool's normal "no results" output

#### Scenario: Empty result after quiescence
- **WHEN** a position query gets an empty result and the language server reported `quiescent: true` both when the query was sent and when the result arrived
- **THEN** the system SHALL return the tool's normal "no results" output
