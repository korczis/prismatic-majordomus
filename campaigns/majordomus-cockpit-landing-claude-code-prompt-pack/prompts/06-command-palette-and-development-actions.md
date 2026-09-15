# PROMPT 06 — Command palette and contextual development actions

## Mission

Make the landing page keyboard-first and immediately operable.

## Command palette

Implement/reuse a canonical command/action palette accessible from landing.

Its entries must derive from existing capability/workflow/command registries.

No giant frontend action array.

Possible discoverable actions include:

```text
open issue
open milestone
resume session
start workflow
run tests
inspect failing gate
open REPL
spawn/request reviewer
show diff
validate repository
```

Only expose actions supported by actual runtime capabilities.

## Context awareness

Palette ranking/availability should use current page/project/task context.

Examples:

- active session makes `resume` prominent
- failing gate makes remediation visible
- selected issue exposes issue workflows

## Search

Support fast search over canonical identifiers/names.

Ordering must be deterministic and useful.

## Keyboard UX

Implement appropriate shortcuts consistent with existing Cockpit UI conventions.

Ensure keyboard navigation, focus management and escape behavior work.

## Cross-surface

Where the same command/workflow exists in CLI/MCP, preserve canonical ID and semantics.

## Tests

Cover:

- registry-derived entries
- context-sensitive availability
- search
- deterministic ranking
- keyboard interaction
- permission/unavailable states

## Acceptance

A developer can open landing and reach common development operations without hunting through navigation.
