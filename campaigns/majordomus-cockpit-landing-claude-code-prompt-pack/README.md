# Majordomus Cockpit Landing Page — Claude Code Prompt Pack

Purpose: turn the Cockpit landing page into a real development control plane rather than a static dashboard.

## How to use

Run the prompts in numeric order. Each prompt assumes the previous phase is implemented, tested, documented, committed, pushed when repository policy requires it, and the repository is left in a clean state.

Recommended execution order:

1. `prompts/00-master-instructions.md`
2. `prompts/01-audit-current-landing-and-data-sources.md`
3. `prompts/02-canonical-landing-view-model.md`
4. `prompts/03-attention-priority-and-resume-engine.md`
5. `prompts/04-actionable-issue-milestone-session-cards.md`
6. `prompts/05-live-events-peers-runtime-health.md`
7. `prompts/06-command-palette-and-development-actions.md`
8. `prompts/07-liveview-interactivity-and-realtime.md`
9. `prompts/08-visual-system-density-responsive-a11y.md`
10. `prompts/09-cli-api-openapi-mcp-surface-parity.md`
11. `prompts/10-derived-docs-gh-pages-and-governance.md`
12. `prompts/11-tests-contracts-determinism-and-drift-gates.md`
13. `prompts/12-end-to-end-deploy-verify-cleanup.md`

## Non-negotiable principles

- One canonical source of truth.
- No Cockpit-only business logic.
- No manually maintained inventories duplicated across HEEx, JS, CLI, API, OpenAPI, MCP, docs.
- Prefer inferred/derived/generated representations over hand-maintained copies.
- Every user-facing collection must be deterministic.
- Landing must answer: what is happening, what needs attention, what can be resumed, what can be acted on.
- Every displayed action must correspond to a real canonical capability or workflow.
- Every important displayed fact must have provenance or a canonical backend source.
- No claims of completion without tests and repository evidence.

