# Design

## Context

See proposal.md (Why). Client capabilities are static JSON built per language in `src/language/typescript.rs` and `src/language/rust.rs` (`client_capabilities()`) and passed by `LanguageServerManager` into `LspClient::start()`. `LspClient::prepare_call_hierarchy()` refuses to send call hierarchy requests unless the stored `ServerCapabilities.call_hierarchy_provider` is enabled. typescript-language-server sets `callHierarchyProvider` only when the client sends `textDocument.callHierarchy` and the bundled TypeScript is >= 3.8; CI pins typescript-language-server 5.2.0 and TypeScript 5.8.3.

The TypeScript fixture (`fixtures/ts-project/index.ts`) is a single script file with no `import`/`export`: `greetEntity` (line 21) is called at lines 26 and 29 from top-level code, and its body calls `entity.greet()` on the `Greetable` interface. `tsconfig.json` includes only `index.ts`. Every existing TypeScript integration test hardcodes line/column positions in that file, so any fixture edit must leave the first 29 lines of `index.ts` byte-for-byte unchanged.

## Goals / Non-Goals

**Goals:**
- Make typescript-language-server advertise call hierarchy with no change to the request path, error handling, or tool output.
- Cover TypeScript `incoming_calls` / `outgoing_calls` in the integration suite.

**Non-Goals:**
- Any change to how `goto_definition` follows TypeScript imports, or to rust-analyzer index readiness (separate issues).
- Supporting `depth > 1`.

## Decisions

**1. Declare `callHierarchy` for Rust as well as TypeScript.**
Add `"callHierarchy": { "dynamicRegistration": false }` under `textDocument` in both capability sets. rust-analyzer advertises call hierarchy regardless, so this changes nothing observable for Rust; it makes the client honest about what it uses and protects against a future rust-analyzer (or a configured replacement command) that gates the provider on the client capability, which is exactly how the TypeScript bug arose.
Alternative: TypeScript only. Smaller diff, but it leaves the Rust path depending on server leniency and keeps the two capability sets inconsistent for no benefit.
`dynamicRegistration: false` matches every other entry; the client does not handle `client/registerCapability`.

**2. Extend the fixture with a second module so the tests cross a file boundary.**
The real-world failure (Issue #28) was `incoming_calls` on a hook imported into other files, so the CI test should cross files too. Changes to `fixtures/ts-project/`, all additive:
- Append `export { greetEntity, User };` as the last line of `index.ts` (after line 29). This turns `index.ts` into a module without touching any existing line, so every hardcoded position in the TypeScript tests stays valid. The top-level `user`/`admin` constants become module-scoped, which no test depends on.
- Add `caller.ts`:
  ```ts
  import { greetEntity, User } from "./index";

  export function useGreeting(): string {
    return greetEntity(new User("Bob", 25));
  }
  ```
- Add `"caller.ts"` to `tsconfig.json` `include` so tsserver puts both files in one project.
Tests (assertions mirror `tests/integration_rust.rs`: `json: true`, non-empty result):
- `test_incoming_calls_cross_file`: `incoming_calls` on `greetEntity` (`index.ts` line 21, col 10, the position `test_rename` already uses). Assert the result is non-empty AND at least one caller's location is in `caller.ts` (match on the file path/URI in the JSON). The same-file top-level call sites may also appear; the test does not depend on them.
- `test_outgoing_calls`: `outgoing_calls` on `useGreeting` (`caller.ts` line 3, col 17). Assert at least one callee; `greetEntity` is a plain function call so tsserver reports it reliably, which avoids the open question of whether an interface-method call (`entity.greet()`) is reported as a callee.
Alternative considered: keep the fixture single-file and test `greetEntity` only. Rejected: it would not exercise the cross-file resolution that failed in practice, and the outgoing assertion would rest on interface-method behaviour that is unverified.

## Risks / Trade-offs

- [Turning `index.ts` into a module could change what an existing test observes (e.g. hover text or diagnostics)] → the appended `export` line is the only edit; if a test breaks, the failure is visible in the TypeScript integration run and the position can be re-checked without moving code.
- [tsserver may need the importing file to be part of the project to report cross-file callers] → `caller.ts` is listed in `tsconfig.json` `include`; the test does not rely on `caller.ts` being opened by the client.
- [The CI fixture is still far smaller than a real multi-package workspace] → keep one manual check on a real workspace with a TypeScript frontend before the release, as a sanity check rather than the only coverage.
- [Users on typescript-language-server < 3.0.0 or TypeScript < 3.8 still get the "does not support call hierarchy" error] → acceptable and correct: the existing error path covers servers that truly lack the feature; no extra handling.
