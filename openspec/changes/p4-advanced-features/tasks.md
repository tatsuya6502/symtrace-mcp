## 1. LspClient Methods

- [x] 1.1 Add `hover` method to `LspClient` — capability check, send `textDocument/hover`, return `Option<Hover>`
- [x] 1.2 Add `diagnostic` method to `LspClient` — capability check, send `textDocument/diagnostic`, return `Vec<Diagnostic>`
- [x] 1.3 Add `rename` method to `LspClient` — capability check, send `textDocument/rename`, return `Option<WorkspaceEdit>`

## 2. MCP Tool Handlers

- [x] 2.1 Add `hover` tool handler and schema — normalize `Hover.contents` for text output, raw `Value` for JSON output
- [x] 2.2 Add `diagnostics` tool handler and schema — format as `line:col [severity] message` for text output
- [x] 2.3 Add `rename` tool handler and schema — format `WorkspaceEdit` as preview, no file mutation

## 3. Tool Registration

- [x] 3.1 Register `hover`, `diagnostics`, and `rename` tools in `McpServer::new`

## 4. Tests

- [x] 4.1 Add LspClient unit tests for hover, diagnostic, and rename methods (mock transport)
- [x] 4.2 Add tool handler tests for hover, diagnostics, and rename (success + error cases)

## 5. Documentation

- [x] 5.1 Update `README.md` and `README.ja.md` — add hover, diagnostics, and rename tools to feature list and MCP Protocol section
- [x] 5.2 Update `CLAUDE.md` — add new tools to relevant sections

## 6. Spec Sync

- [ ] 6.1 Archive completed change — merge spec deltas into `openspec/specs/lsp-client/spec.md` and `openspec/specs/tools-definitions/spec.md`
