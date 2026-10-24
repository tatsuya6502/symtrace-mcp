# lsp-transport Specification

## Purpose
Implements JSON-RPC 2.0 over stdio with `Content-Length` framing to a language server child process, resolving responses to pending requests and forwarding server notifications over a channel.

## Requirements

### Requirement: LSP transport sends JSON-RPC requests with Content-Length framing
The transport SHALL encode outgoing messages as `Content-Length: <N>\r\n\r\n<json-body>` where N is the byte length of the UTF-8 encoded JSON body.

#### Scenario: Send a request to a language server
- **WHEN** the transport sends a JSON-RPC request
- **THEN** it writes `Content-Length: <N>\r\n\r\n` followed by the JSON body to the child process stdin

### Requirement: LSP transport reads JSON-RPC responses with Content-Length framing
The transport SHALL read incoming messages by parsing the `Content-Length` header, then reading exactly that many bytes as the JSON body. For messages with an `id` field (responses), the transport SHALL resolve the pending request. For messages with a `method` field and no `id` (notifications), the transport SHALL forward them via an mpsc channel. For messages with both `id` and `method` (server-initiated requests), the transport SHALL respond with `-32601` (unsupported). The transport SHALL accept optional environment variables to augment the child process environment.

#### Scenario: Receive a response from a language server
- **WHEN** the transport reads from the child process stdout
- **THEN** it parses the `Content-Length` header, reads N bytes, and deserializes the JSON body

#### Scenario: Receive a server notification
- **WHEN** the transport reads a message with a `method` field and no `id` field
- **THEN** it SHALL send `(method, params)` through the notification mpsc channel

#### Scenario: Notification channel dropped (client shut down)
- **WHEN** the transport tries to send a notification but the receiver has been dropped
- **THEN** the transport SHALL continue processing other messages without error

#### Scenario: Spawn with environment variables
- **WHEN** `LspTransport::spawn()` is called with environment variables
- **THEN** the child process SHALL inherit the parent environment with the specified variables added or overridden

#### Scenario: Spawn without environment variables
- **WHEN** `LspTransport::spawn()` is called with no environment variables
- **THEN** the child process SHALL inherit the parent environment unchanged

### Requirement: LSP transport routes responses by request ID
The transport SHALL maintain a map of pending request IDs to oneshot channels. When a response arrives, it SHALL route it to the correct channel by ID.

#### Scenario: Response matches a pending request
- **WHEN** a JSON-RPC response with id=42 arrives and there is a pending oneshot channel for id=42
- **THEN** the response body is sent through that oneshot channel

#### Scenario: Response has no matching pending request
- **WHEN** a JSON-RPC response arrives with an ID not in the pending map
- **THEN** the response is logged and discarded

### Requirement: LSP transport spawns a background reader task
The transport SHALL spawn a tokio task that continuously reads from the language server stdout and routes responses and notifications.

#### Scenario: Language server sends interleaved messages
- **WHEN** the language server sends a notification between two responses
- **THEN** both responses are routed to their respective channels and the notification is logged

### Requirement: LSP transport detects language server process exit
The transport SHALL detect when the child process exits and return an error on any pending operations.

#### Scenario: Language server crashes
- **WHEN** the child process exits unexpectedly
- **THEN** all pending request channels receive an error and the reader task terminates

### Requirement: LSP transport sends notifications without expecting responses
The transport SHALL support sending JSON-RPC notifications (messages without an ID) for fire-and-forget operations like `textDocument/didOpen`.

#### Scenario: Send a notification
- **WHEN** the transport sends a notification (no request ID)
- **THEN** the message is written to stdin and no pending channel is created
