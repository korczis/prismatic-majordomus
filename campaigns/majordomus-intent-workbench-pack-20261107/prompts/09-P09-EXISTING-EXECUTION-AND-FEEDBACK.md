# P09: Existing Execution And Feedback

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
Materialized issues have a safe **Start work** path through existing `plan.next` → `proceed.step` → `devtask.implement` and guarded lifecycle; the Workbench displays what happened and the evidence-backed intent verdict. It must not invent an executor, new issue status or separate mesh.

## Inspect first
Read `docs/PROCEED.md`, `docs/EXECUTIONS.md`, `docs/MESH.md`, `apps/majordomus-cli/src/capability/builtin/proceed.rs`, existing `ui/idd.rs` moves and execution routes, relevant integration tests and current dependency status. Verify actual available commands, no stale assumptions from this pack.

## Implement one vertical
- Make the approved/materialized intent link to its canonical issue(s) and existing IDD work and move panels. If the existing UI already exposes safe moves, reuse it instead of adding a second button with different semantics.
- Where permitted, offer an explicit **Start next ready issue** action wired to existing `proceed.step` and `devtask.implement` execution. Respect readiness, scope, exclusive mesh claim, idempotency and policy; never start blocked issue, and never let editing trigger dispatch.
- Observe events through existing live stream/monitor. On each update re-read canonical derived state, show recorded evidence versus mere activity, and explain exactly why intent criteria are still unmet or now satisfied.
- Tests: same key double click one execution, no dispatch of blocked issue, duplicate peer claim refusal, failed tests never mark criterion satisfied, handover/execution link survives reload, one real browser flow with fixture.

## Proof / next
A canonical ready issue can be launched or navigated to existing runner once from the Workbench and later inspected from the same intent. No claim of successful release until the actual release/deployment observation exists. Land a narrow, nonduplicated execution integration.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.
