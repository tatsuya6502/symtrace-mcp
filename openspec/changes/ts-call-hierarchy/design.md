# Design

## Context

See proposal.md (Why). Client capabilities are static JSON built per language in `src/language/typescript.rs` and `src/language/rust.rs` (`client_capabilities()`) and passed by `LanguageServerManager` into `LspClient::start()`. `LspClient::prepare_call_hierarchy()` refuses to send call hierarchy requests unless the stored `ServerCapabilities.call_hierarchy_provider` is enabled. typescript-language-server sets `callHierarchyProvider` only when the client sends `textDocument.callHierarchy` and the bundled TypeScript is >= 3.8; CI pins typescript-language-server 5.2.0 and TypeScript 5.8.3.

The TypeScript fixture (`fixtures/ts-project/index.ts`) already has a small call graph: `greetEntity` (line 21) is called at lines 26 and 29 from top-level code, and its body calls `entity.greet()` on the `Greetable` interface. Every existing TypeScript integration test hardcodes line/column positions in that file.

## Goals / Non-Goals

**Goals:**
- Make typescript-language-server advertise call hierarchy with no change to the request path, error handling, or tool output.
- Cover TypeScript `incoming_calls` / `outgoing_calls` in the integration suite.

**Non-Goals:**
- Cross-file call hierarchy assertions in the fixture (checked manually on a real workspace instead; see Risks).
- Any change to how `goto_definition` follows TypeScript imports, or to rust-analyzer index readiness (separate issues).
- Supporting `depth > 1`.

## Decisions

**1. Declare `callHierarchy` for Rust as well as TypeScript.**
Add `"callHierarchy": { "dynamicRegistration": false }` under `textDocument` in both capability sets. rust-analyzer advertises call hierarchy regardless, so this changes nothing observable for Rust; it makes the client honest about what it uses and protects against a future rust-analyzer (or a configured replacement command) that gates the provider on the client capability, which is exactly how the TypeScript bug arose.
Alternative: TypeScript only. Smaller diff, but it leaves the Rust path depending on server leniency and keeps the two capability sets inconsistent for no benefit.
`dynamicRegistration: false` matches every other entry; the client does not handle `client/registerCapability`.

**2. Test against the existing fixture call graph, append-only if needed.**
- `test_incoming_calls`: `incoming_calls` on `greetEntity` (line 21, col 10, the position `test_rename` already uses). Expect at least one caller; tsserver reports the top-level call sites under a file/module-level caller item, so the assertion is on count, not caller name.
- `test_outgoing_calls`: `outgoing_calls` on `greetEntity`, expecting at least one callee (`entity.greet()`).
- Assertions mirror `tests/integration_rust.rs` (`json: true`, `!arr.is_empty()`).
Alternative considered: rewrite the fixture with a richer, multi-file call graph. Rejected because every TypeScript test depends on current line numbers, and the `tsconfig.json` `include` lists only `index.ts`.
Fallback: if tsserver returns no outgoing call for the interface method call, append (never insert) a named function at the end of `index.ts` that calls `greetEntity` and `new User(...)`, and point `test_outgoing_calls` at it. Appending keeps all existing positions valid.

## Risks / Trade-offs

- [tsserver may not resolve `entity.greet()` on an interface method signature as a call hierarchy callee, giving an empty outgoing result] → use the append-only fallback in Decision 2.
- [The fixture is a single script file, so cross-file callers (the real-world failure: an imported React hook) are not exercised in CI] → manual verification task on a real multi-package workspace with a TypeScript frontend: `incoming_calls` on an imported hook must return its callers in other files.
- [Users on typescript-language-server < 3.0.0 or TypeScript < 3.8 still get the "does not support call hierarchy" error] → acceptable and correct: the existing error path covers servers that truly lack the feature; no extra handling.
