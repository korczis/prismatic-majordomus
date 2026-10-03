# Execution Notes for Claude Code Fable / Opus

## Context strategy

Use the large context window to read complete architectural neighborhoods, not isolated files. Prioritize canonical project instructions and repository evidence.

Recommended context ingestion order:

1. `AGENTS.md` and root docs
2. rules/doctrines/schemas
3. `.ai/**` and `.majordomus/**`
4. CLI runtime/environment modules
5. server/API/OpenAPI/MCP modules
6. Cockpit backend/frontend
7. session-context/handover/worktree modules
8. tests/CI/gates
9. docs/site generation
10. git history for relevant subsystems if needed to understand competing legacy designs

## Worktree discipline

Follow the repository's enforced worktree/branch doctrine. Do not improvise a worktree layout.

## Evidence discipline

For each stage retain:

- files inspected,
- root causes discovered,
- decisions and why,
- tests added,
- exact commands executed,
- failures encountered,
- legacy removed,
- repository gates passed.

## Scope discipline

Adjacent refactors are justified only when necessary to remove duplication, make the invariant enforceable, or prevent a known bypass. Do not redesign unrelated subsystems because the context window is large and temptation is apparently a feature of intelligent systems.
