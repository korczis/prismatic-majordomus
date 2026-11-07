# P00: Land The Entrypoint

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
A real user can navigate from the existing IDD workspace to a **New Intent / Compose** draft surface and back. The surface renders a useful, accessible editor with empty/filled states and a clear **Draft, NOT an intent record** label. It never writes the canonical tree and must be discoverable by the existing UI navigation/audit. Reuse current UI templates, design tokens, routing and capabilities; do not create a second UI application or React island.

## Inspect first
Read `apps/majordomus-cli/src/ui/idd.rs`, `ui/mod.rs`, `ui/source.rs`, `docs/UI.md`, `docs/COCKPIT.md`, `test/cases/3220_the_idd_loop_is_walked_in_a_browser.sh`, any existing design/translation templates and the relevant issue/PR scopes. Check whether a compose route already exists: extend it if so.

## Implement exactly one visible slice
- Add the narrowest actual route/state to reach composition from `/ui/idd`; favor a parameterized view of the current workspace unless routing conventions demand otherwise.
- Render a multiline draft entry with guidance (what must become true, constraints, what counts as evidence), a status explaining **no tracked write yet**, back navigation, semantic `label` and keyboard focus.
- HTML-escape user text in all render paths; never interpret pasted Markdown as executable HTML.
- No fake AI calls, no placeholder 'Analyze' success, no server write from editor typing. The draft may be session-ephemeral here; P01 owns persistence.
- Provide test coverage for route discovery, default/empty state, XSS text escaping, keyboard/label semantics, and absence of repository mutation.

## Proof / next
Run the affected UI tests plus `scripts/ui check`, show the actual served URL and screenshot/browser proof if probe is available, land PR. If one of the relevant existing issues already covers this exactly, extend its acceptance without opening a duplicate. **First deliverable: a navigable real New Intent editor on master.**


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.
