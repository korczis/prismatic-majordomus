# P01: Private Draft And Revision

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
A person types a free-form, multi-paragraph idea in the New Intent surface, refreshes or disconnects, and resumes the **same draft**. They can inspect and restore a prior revision. No edit touches `.ai/repo/project/intents`, `campaigns/`, the public site or GitHub.

## Reuse before adding anything
Inspect `.ai/local` state conventions, workspaces/continuity, existing atomic state writers and `docs/CONTINUITY.md`; inspect UI request/CSRF/origin and authentication rules. Prefer an existing local-draft writer. A new `intent-workbench` type is a private UI working projection, NOT a new repository object kind, plan noun or canonical intent status. If a new typed capability is necessary, declare it once through the canonical registry and ensure correct `process_state`/local-only or policy-defined effect.

## Implement one vertical
- Local-only saved draft, explicit owner namespace, random stable draft ID, `revision`, content digest, created/updated metadata, bounded text size; atomic write/recovery. No raw content in log/telemetry, Git-tracked docs, or static snapshots.
- Version/CAS guard prevents lost updates. Repeated save with the same revision/content is idempotent; concurrent incompatible edits return conflict with both revisions. Debounce UI writes; save state labels must be real (saving/saved/error/offline).
- Read/resume from same trusted local runtime. No remote access to private content merely because HTTP is exposed; verify origin/CSRF/auth constraints and forbid path traversal, ID enumeration and credential leaks.
- Add focused tests: refresh resumes, two tabs conflict rather than clobber, repeated save no duplicate, content bounds, unsafe markup escapes, server restart persistence, static publication fixture contains no user draft canary.

## Proof / next
Show an actual browser workflow: write, refresh, restart if relevant, reopen, restore; test non-write invariants. Deliver tested versioned persistence, then P02 can safely use it. A localStorage fallback may be used ONLY if existing repository policy requires it, and must not be misrepresented as shared or durable across devices.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.
