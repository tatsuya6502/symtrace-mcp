## ADDED Requirements

### Requirement: LspClient caches push diagnostics via moka future::Cache
The system SHALL maintain a `moka::future::Cache<String, Vec<Diagnostic>>` on each `LspClient` instance, keyed by file URI. The cache SHALL have a configurable TTL (default 600 seconds, matching idle timeout). When the cache expires or the server restarts (idle shutdown), cached diagnostics are discarded.

#### Scenario: Push diagnostics cached on notification
- **WHEN** `LspClient` receives a `textDocument/publishDiagnostics` notification with a URI and diagnostics list
- **THEN** the system SHALL insert or replace the entry for that URI in the moka cache

#### Scenario: Cache entry expires after TTL
- **WHEN** a cache entry's TTL elapses without being updated
- **THEN** the entry SHALL be automatically evicted by moka

#### Scenario: Cache clears on server restart
- **WHEN** a language server shuts down due to idle timeout and a new instance starts
- **THEN** the new `LspClient` instance SHALL have an empty diagnostics cache

### Requirement: Diagnostics cache invalidated on file change
The system SHALL invalidate the cache entry for a URI when `did_change` or `did_open` is called for that URI. This prevents stale pre-edit diagnostics from being returned after a file modification.

#### Scenario: Cache invalidated on didChange
- **WHEN** `did_change(uri)` is called (file modified on disk)
- **THEN** the system SHALL invalidate the moka cache entry for that URI via `cache.invalidate(&uri)`

#### Scenario: Cache invalidated on didOpen
- **WHEN** `did_open(uri)` is called (file opened for the first time)
- **THEN** the system SHALL invalidate the moka cache entry for that URI

#### Scenario: Other files' cache entries unaffected
- **WHEN** `did_change` is called for URI "file:///src/App.tsx" and the cache also has entries for "file:///src/utils.ts"
- **THEN** only the entry for "file:///src/App.tsx" SHALL be invalidated; "file:///src/utils.ts" remains cached

### Requirement: LspClient::diagnostic is capability-aware
The system SHALL check the server's `diagnosticProvider` capability when `diagnostic(uri)` is called. If the server supports pull diagnostics, the system SHALL send `textDocument/diagnostic`. If the server does not support pull diagnostics, the system SHALL read from the moka cache. If the cache has no entry for the URI, the system SHALL return an empty `Vec<Diagnostic>`.

#### Scenario: Pull diagnostics server (e.g., rust-analyzer)
- **WHEN** `diagnostic(uri)` is called and the server's capabilities include `diagnosticProvider`
- **THEN** the system SHALL send `textDocument/diagnostic` and return the parsed diagnostics (unchanged behavior)

#### Scenario: Push-only server with cached diagnostics (e.g., typescript-language-server)
- **WHEN** `diagnostic(uri)` is called and the server's capabilities do not include `diagnosticProvider`
- **THEN** the system SHALL read from the moka cache for the given URI and return the cached diagnostics

#### Scenario: Push-only server with cache miss
- **WHEN** `diagnostic(uri)` is called, the server does not support pull diagnostics, and the moka cache has no entry for the URI
- **THEN** the system SHALL return an empty `Vec<Diagnostic>`
