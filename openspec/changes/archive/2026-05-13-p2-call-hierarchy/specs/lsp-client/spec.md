## ADDED Requirements

### Requirement: Call hierarchy protocol methods
The system SHALL provide `prepare_call_hierarchy`, `incoming_calls`, and `outgoing_calls` methods on `LspClient`. These implement the two-step LSP callHierarchy protocol: `prepareCallHierarchy` resolves a position to a `CallHierarchyItem`, then `incomingCalls`/`outgoingCalls` retrieve the call relationships.

#### Scenario: prepareCallHierarchy returns items
- **WHEN** `prepare_call_hierarchy` is called with a valid URI and position
- **THEN** the system SHALL send `textDocument/prepareCallHierarchy` and return the resulting `CallHierarchyItem` list

#### Scenario: prepareCallHierarchy returns null
- **WHEN** `prepare_call_hierarchy` returns `null` or an empty array
- **THEN** the system SHALL return an empty list (not an error)

#### Scenario: incomingCalls returns callers
- **WHEN** `incoming_calls` is called with a `CallHierarchyItem`
- **THEN** the system SHALL send `callHierarchy/incomingCalls` with the item and return `CallHierarchyIncomingCall` results

#### Scenario: outgoingCalls returns callees
- **WHEN** `outgoing_calls` is called with a `CallHierarchyItem`
- **THEN** the system SHALL send `callHierarchy/outgoingCalls` with the item and return `CallHierarchyOutgoingCall` results

#### Scenario: incomingCalls or outgoingCalls returns null
- **WHEN** an incoming/outgoing call query returns `null` or an empty array
- **THEN** the system SHALL return an empty list (not an error)

### Requirement: Call hierarchy capability check
The system SHALL check `ServerCapabilities.call_hierarchy_provider` before sending any callHierarchy requests. If the capability is absent (`None`), the system SHALL return a protocol error indicating the language server does not support call hierarchy.

#### Scenario: Language server supports call hierarchy
- **WHEN** `call_hierarchy_provider` is present in server capabilities
- **THEN** the system SHALL proceed with callHierarchy protocol requests

#### Scenario: Language server does not support call hierarchy
- **WHEN** `call_hierarchy_provider` is `None` in server capabilities
- **THEN** the system SHALL return a `ClientError::Protocol` with message indicating call hierarchy is not supported
