## 1. LSP Client

- [ ] 1.1 Create `src/lsp/client.rs` with `LspClient` struct owning `LspTransport`, `root_uri`, `open_files: HashSet<String>`, and `capabilities: ServerCapabilities`
- [ ] 1.2 Implement `LspClient::start` — spawn child process via `LspTransport::spawn`, send `initialize` with client capabilities and root URI, receive `InitializeResult`, send `initialized` notification
- [ ] 1.3 Implement `LspClient::shutdown` — send `shutdown` request, wait for response, send `exit` notification
- [ ] 1.4 Add rust-analyzer–specific initialization parameters in `src/language/rust.rs` (hover capability, file operations, etc.)
- [ ] 1.5 Implement index readiness wait — poll `textDocument/documentSymbol` until non-empty result or timeout

## 2. File Management

- [ ] 2.1 Create `src/lsp/file_manager.rs` with `FileManager` struct tracking `open_files: HashMap<String, OpenFile>` (URI → version + mtime)
- [ ] 2.2 Implement `FileManager::ensure_open` — read file from disk, send `didOpen` if new or `didChange` if mtime changed, update tracking
- [ ] 2.3 Implement `FileManager::close` — send `didClose` and remove from tracking

## 3. Server Manager

- [ ] 3.1 Create `src/server/manager.rs` with `LanguageServerManager` struct holding configs, clients map, file managers map, idle monitor, and root path
- [ ] 3.2 Implement `get_client_for_file` — resolve file extension to language, start server lazily if needed, return guarded client reference
- [ ] 3.3 Implement `start_server` — create `LspClient`, call `start`, store in clients map
- [ ] 3.4 Implement `stop_server` — call `LspClient::shutdown`, remove from clients map, clean up file manager

## 4. Idle Monitor

- [ ] 4.1 Create `src/server/idle_monitor.rs` with `IdleMonitor` struct holding last-used timestamps, timeout, and check interval
- [ ] 4.2 Implement `IdleMonitor::run` as a background tokio task that periodically checks for idle servers and shuts them down
- [ ] 4.3 Implement `IdleMonitor::touch` to update last-used timestamp on each tool invocation

## 5. MCP Tools

- [ ] 5.1 Define tool schemas for `find_references`, `goto_definition`, `find_implementations` with parameters (`file_path`, `line`, `column`, optional `json`)
- [ ] 5.2 Implement `find_references` handler — resolve file, ensure open, send `textDocument/references`, format output
- [ ] 5.3 Implement `goto_definition` handler — resolve file, ensure open, send `textDocument/definition`, format output
- [ ] 5.4 Implement `find_implementations` handler — resolve file, ensure open, send `textDocument/implementation`, format output
- [ ] 5.5 Implement human-readable output formatting (spec §6.5: `file:line:col  line_text` with summary)
- [ ] 5.6 Implement JSON output formatting (`json: true` parameter)
- [ ] 5.7 Implement error handling — invalid path, unsupported language, server errors

## 6. Wiring

- [ ] 6.1 Wire `LanguageServerManager` into `McpServer` — create on startup, pass to tool handlers
- [ ] 6.2 Register the three tools in `McpServer::new` with their schemas
- [ ] 6.3 Wire `IdleMonitor` — spawn background task on server start, pass manager reference
- [ ] 6.4 Implement graceful shutdown — stop all language servers when MCP event loop ends
