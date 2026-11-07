# P06: Reconcile Duplicates Impact

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
Before any materialization, every proposed requirement is reconciled with what the repository **already knows**. Display the structured classification and cited reasons: implemented/evidenced, already planned, partial, novel, conflicting, superseded, unknown. Never generate one issue per sentence.

## Existing work to finish, not fork
Inspect I3251, I3254 (#1591), I3273 (#1605) and their dependencies plus `a-brief-is-reconciled-into-the-plan` and the current duplicate/object-search implementation. I3254 currently describes duplicate/refusal behavior; I3273 describes revision segment classification. If blocked, land genuinely unblocked predecessor and explicitly report dependency, not a bypass capability. Reuse existing duplicate detection as soon as it exists.

## Implement one vertical
- Stable requirement identity derived from normalized text/digest under current plan conventions, with source location separate from content identity. Preserve exact original draft as private source, not governance.
- Query canonical intent statements/criteria, issues, evidence verdict, supersession, architecture decisions and relevant code. Classify with explicit matched identity, source revision, reason and confidence/unknown; classifier suggestion cannot mark satisfaction itself.
- Deduplicate near-equivalent asks; opt to amend existing records rather than append. No direct mutation from this step. Novelty override must carry a reason and be visible later to `draft_check`.
- For a known conflict with an invariant or ADR, surface both cited texts and require a real human governance decision before promoting an incompatible plan.
- Tests: known duplicate across different words, exact duplicate under new ID, already satisfied criterion from existing verdict, partial coverage, conflict, unknown due to stale source, stable classification under reordered sources.

## Proof / next
Dogfood with an intent overlapping `intent-planning`, show the existing canonical record it overlaps and prevent creation of a duplicate. Land issue evidence on master. **No classification based solely on model persuasion.**


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.
