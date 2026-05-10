## ADDED Requirements

### Requirement: Core position and range types
The system SHALL define `Position { line: u32, character: u32 }` (0-based) and `Range { start: Position, end: Position }` matching the LSP specification.

#### Scenario: Deserialize a position from JSON
- **WHEN** a JSON `{"line": 10, "character": 5}` is deserialized
- **THEN** it produces `Position { line: 10, character: 5 }`

### Requirement: Location type
The system SHALL define `Location { uri: String, range: Range }` for representing a position within a document.

#### Scenario: Deserialize a location
- **WHEN** a JSON `{"uri": "file:///src/main.rs", "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 10}}}` is deserialized
- **THEN** it produces the corresponding `Location` struct

### Requirement: Text document identifier types
The system SHALL define `TextDocumentIdentifier { uri: String }` and `VersionedTextDocumentIdentifier { uri: String, version: i32 }`.

#### Scenario: Deserialize a text document identifier
- **WHEN** a JSON `{"uri": "file:///src/foo.rs"}` is deserialized
- **THEN** it produces the corresponding `TextDocumentIdentifier`

### Requirement: Initialize request and response types
The system SHALL define `InitializeParams` (with `rootUri`, `capabilities`, `processId`) and `InitializeResult` (with `capabilities: ServerCapabilities`).

#### Scenario: Serialize initialize params
- **WHEN** an `InitializeParams` is serialized to JSON
- **THEN** the output contains `rootUri`, `capabilities`, and `processId` fields

### Requirement: ServerCapabilities type
The system SHALL define `ServerCapabilities` with optional fields for the capabilities relevant to symtrace-mcp: `referencesProvider`, `definitionProvider`, `implementationProvider`, `hoverProvider`, `renameProvider`, `callHierarchyProvider`, `diagnosticProvider`.

#### Scenario: Deserialize server capabilities from rust-analyzer
- **WHEN** an initialize response from rust-analyzer is deserialized
- **THEN** `ServerCapabilities` captures which features the language server supports

### Requirement: Text document content change types
The system SHALL define `TextDocumentContentChangeEvent` with `range` (optional) and `text` fields for supporting `textDocument/didChange`.

#### Scenario: Full document content change
- **WHEN** a change event without a range is deserialized
- **THEN** it represents a full document replacement

### Requirement: Hover and diagnostic types
The system SHALL define `Hover { contents: Value, range: Option<Range> }` and `Diagnostic { range: Range, severity: Option<i32>, message: String }` for future P4 tool support.

#### Scenario: Deserialize a diagnostic
- **WHEN** a JSON diagnostic with range, severity, and message is deserialized
- **THEN** it produces the corresponding `Diagnostic` struct

### Requirement: Call hierarchy types
The system SHALL define `CallHierarchyItem` and `CallHierarchyIncomingCall` / `CallHierarchyOutgoingCall` for P2 callHierarchy support.

#### Scenario: Deserialize an incoming call
- **WHEN** a callHierarchy/incomingCalls response is deserialized
- **THEN** it produces a list of `CallHierarchyIncomingCall` items

### Requirement: Workspace edit type
The system SHALL define `WorkspaceEdit` and `TextEdit` for P4 rename support.

#### Scenario: Deserialize a workspace edit
- **WHEN** a textDocument/rename response is deserialized
- **THEN** it produces a `WorkspaceEdit` with file-to-changes mapping
