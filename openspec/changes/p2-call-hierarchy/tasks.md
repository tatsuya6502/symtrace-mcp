## 1. LspClient call hierarchy methods

- [ ] 1.1 Add `prepare_call_hierarchy(&self, uri, position) -> Result<Vec<CallHierarchyItem>>` to `LspClient` in `src/lsp/client.rs`
- [ ] 1.2 Add `incoming_calls(&self, item: &CallHierarchyItem) -> Result<Vec<CallHierarchyIncomingCall>>` to `LspClient`
- [ ] 1.3 Add `outgoing_calls(&self, item: &CallHierarchyItem) -> Result<Vec<CallHierarchyOutgoingCall>>` to `LspClient`
- [ ] 1.4 Add capability check: return `ClientError::Protocol` if `ServerCapabilities.call_hierarchy_provider` is `None`

## 2. MCP tool handlers

- [ ] 2.1 Add `incoming_calls_schema()` and `outgoing_calls_schema()` to `src/mcp/handlers.rs`
- [ ] 2.2 Implement `incoming_calls` handler: parse params, validate `depth`, call `prepare_call_hierarchy` → `incoming_calls`, format output
- [ ] 2.3 Implement `outgoing_calls` handler: parse params, validate `depth`, call `prepare_call_hierarchy` → `outgoing_calls`, format output
- [ ] 2.4 Add text format for call hierarchy results (← for callers, → for callees) and JSON format with `name` field
- [ ] 2.5 Add `depth` validation: reject values other than `1` with MCP error `-32602`

## 3. Tool registration

- [ ] 3.1 Register `incoming_calls` and `outgoing_calls` tools in `McpServer::new()` in `src/mcp/tools.rs`

## 4. Testing

- [ ] 4.1 Verify compilation and existing tests still pass (`cargo test`)
- [ ] 4.2 Manual test: `incoming_calls` and `outgoing_calls` against a Rust project with rust-analyzer
