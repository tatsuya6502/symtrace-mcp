## MODIFIED Requirements

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
