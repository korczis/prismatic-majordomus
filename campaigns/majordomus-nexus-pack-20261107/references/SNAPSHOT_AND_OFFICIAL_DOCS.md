# Evidence baseline and official integration references

## Historical repository snapshot supplied with this task

Source: `prismatic-majordomus-context-20260911.zip`, archived on 2026-09-11. It is **not** evidence of the current November master. Verify every entry in the actual checkout before editing.

Observed in that snapshot:

- `.claude/settings.json` already registers `UserPromptSubmit`, `SessionStart`, `SessionEnd`, `PreCompact`.
- `.claude/hooks/majordomus-capture`, `majordomus-session-start`, `majordomus-session-compact`, `majordomus-session-end` exist; they delegate to `majordomus capture ...`.
- `lib/capture.sh` documents immutable raw prompt evidence, JSON+Markdown projection, failure log and that capture must not lose human input; `.ai/local/prompts` is **not** KB.
- `.ai/manifest.yaml` explicitly enumerates canonical sections. Discovery is manifest-driven, not unrestricted directory walking.
- `.ai/repo/policy.yaml` defines minimum-sufficient context and the session briefing/checkpoint/handover policy.
- `CLAUDE.md`/`AGENTS.md` say they are generated from `.ai/repo/policy.yaml`; providers use `majordomus context resolve` and peer announcements.
- `docs/EXECUTIONS.md` states capability executions were in-process, non-durable at the snapshot date; do not assume that remains true.
- `docs/PLANNING.md` describes milestone/issue graph with derived status, not hand-authored `status`.
- `docs/MCP.md` describes shared server and canonical capability registry (MCP, HTTP, OpenAPI, CLI, Cockpit).
- `.ai/repo/workflows/task-lifecycle.md` describes `start`, `checkpoint`, `check`, `finish`, evidence, worktree rules and overlap checks.
- `apps/majordomus-cli/src/capability/builtin/` contains typed capability declarations used for generated surfaces.
- `scripts/collision-check`, `scripts/generate-site-data`, `.just/` modules and `.githooks/` are present.

## Current vendor docs (check installed version)

- Hook event reference and response/decision schemas: https://code.claude.com/docs/en/hooks
- Configuration and scope/precedence: https://code.claude.com/docs/en/settings
- Claude Code plugin/mod documentation index: https://code.claude.com/docs/llms.txt
- Monitoring and OpenTelemetry documentation: https://code.claude.com/docs/en/monitoring-usage

Verified important semantic constraints from official docs (retrieved 2026-11-07):

1. `UserPromptSubmit` can block/add `additionalContext` but cannot replace the original user prompt.
2. `PreToolUse` can deny a tool call; `PostToolUse` occurs after the effect and cannot undo it.
3. `Stop` continuation requires guards such as `stop_hook_active` and bounded attempts.
4. Hook configuration scope and cloud/managed-settings behavior matter; project hooks are not universally enforceable across every hosting mode.
5. A hook's successful process exit is not proof of a valid response schema or of a completed execution.
6. Plugins/mods may enrich integration, but must not be assumed installed or supported without checking.

Keep the docs linked for Claude Code to revalidate at implementation time. Do not auto-copy vendor examples that bypass the repository's canonical generator.
