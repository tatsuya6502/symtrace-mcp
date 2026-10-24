# Spec Delta

## ADDED Requirements

### Requirement: TypeScript first-query cross-file results
TypeScript integration tests SHALL verify that the first tool call on a freshly started server returns project-wide results. These tests SHALL make the cross-file query the first `tools/call` of the session, with no readiness poll or other query before it, so that the call is issued while the semantic project is still loading.

#### Scenario: First goto_definition resolves into the defining file
- **WHEN** the first tool call of a fresh session is `goto_definition` with `json: true` on the `greetEntity` call in `fixtures/ts-project/caller.ts` (line 4, column 10)
- **THEN** the first result SHALL have a file path ending in `index.ts` and line 21

#### Scenario: First find_references spans both files
- **WHEN** the first tool call of a fresh session is `find_references` with `json: true` on the same position
- **THEN** the response SHALL contain at least 6 references, at least one of them in `index.ts`
