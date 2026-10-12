# PHASE 03 — PRODUCTION ZED PROJECTION

Execute master and previous phase constraints.

Build the first complete adapter targeting `.zed/settings.json` and `.zed/tasks.json`.

1. Generate only supported, meaningful project-level overrides for detected languages, actual toolchain/formatters, available LSP configurations, validated schema paths, file associations and generated path exclusions.
2. Build tasks from canonical workflow IDs and commands; preserve namespacing, parameters and repo-local execution context; use safe shell/args handling, quote paths carefully, reject unresolved commands, no copy/pasted shell scripts.
3. Support Zed default behavior: omit redundant overrides; do not over-specify formatter/LSP or disable valid defaults.
4. Validate actual JSON schema associations (JSON, YAML); pay attention to relative worktree paths, rule priority, glob semantics, extensions, and schema availability. Emit diagnosis rather than fake association on missing schema.
5. Inspect Zed agent instruction precedence and supported MCP/ACP configuration. Reuse existing canonical instructions if safe and proven; do not duplicate AGENTS.md/CLAUDE.md as a competing source; do not emit secrets or arbitrary provider selection.
6. Generate stable JSON, with consistent ordering, no unsupported keys and no absolute paths pinned to this machine.
7. Add render/diff/check/explain functions to adapter and exact golden semantics tests plus official-feature contract tests.
8. Demonstrate real Majordomus project output and show which source produced each field.

When uncertain of Zed compatibility, verify official docs/installed Zed, skip unsupported projection with diagnostic rather than guess. Proceed to phase 04.
