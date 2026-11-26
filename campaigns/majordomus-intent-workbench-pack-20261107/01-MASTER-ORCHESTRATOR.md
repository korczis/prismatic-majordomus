# MAJORDOMUS: Live Intent Workbench | MASTER CLAUDE CODE ORCHESTRATOR

## Mission
Turn the existing `/ui/idd` into a real working interface where an owner opens **New Intent**, writes free-form text, receives incremental grounded context from `.ai/`, KB, ADRs, git, history, existing intents/issues and reasoning, discusses only meaningful uncertainty, sees an evolving prompt/atomic prompt-pack preview, and explicitly promotes approved proposals into the canonical intent → milestone → issue → execution lifecycle. Evidence, governance and delivery remain owned by existing Majordomus systems.

**Optimize time-to-first-use, not theoretical architecture coverage.** The *first usable checkpoint* is: new draft → safe autosave → grounded context → a genuine reasoning/assistant answer when a provider exists → copyable prompt preview, all on a real runtime route, without touching tracked records. The *second usable checkpoint* is deduplicated, checked, explicitly authorized materialization and the existing execution path. Each slice must be committed, integrated, tested and documented before building the next dependent slice; some independent slices may run in separate non-overlapping worktrees if claims and scope allow it.

## What has already been verified (re-check live HEAD before acting)
- `apps/majordomus-cli/src/ui/idd.rs` serves `/ui/idd` from existing `intents.list`, `plan.roadmap`, `plan.issues`, `plan.waves`, `plan.next`, plus standing/execution projections. Extend, do not replace.
- `apps/majordomus-cli/src/capability/builtin/intents.rs` owns `intents.propose`; proposed intents do not automatically establish a plan.
- `docs/PLANNING.md` states milestones and issues are canonical YAML records; lifecycle state and waves are derived and evidence-gated.
- `docs/REASONING.md` describes provider-independent, evidence-first decisions and advisory consultations, NOT a blanket authorization for model output to write files.
- `docs/OWNER_PACKS.md`: received owner prompt packs are immutable sources under `campaigns/` and reconciled into the existing plan, not directly executed. `campaigns/` is **not** a chat transcript store.
- Existing intent `a-brief-is-reconciled-into-the-plan` forbids a second intake noun/store and treats external briefs as untrusted.
- Existing intent `what-a-workspace-decides-reaches-the-repository-with-its-source` requires workspace-only private content and explicit promotion, and rejects publishing conversation bytes.
- Existing intent `intent-planning` enforces criterion coverage and critique before execution.

## Existing issues to reconcile, rather than recreate
- I3251: owner brief / existing-context picture; GitHub #1588.
- I3252: `intents.draft_check` + checked write (documented target, NOT assumed landed); GitHub #1589, observed open/blocked.
- I3254: duplicate/overlap detection; GitHub #1591, observed open/blocked.
- I3272–I3275: campaign revision, requirement reconciliation, diff, provenance; GitHub #1604–#1607, observed blocked.
- I3281–I3286: workspace provenance, local-only resolution and authorized remote promotion; avoid treating blocked remote permissions as already solved.
- I4484: `/ui/idd` gap and critique; GitHub #2872, observed open/blocked.
- Inspect I3247, I3167/I3169 and all current dependencies before enabling checked materialization.

## Work sequencing
Run the prompts in `prompts/` in filename order, prioritizing the fastest usable vertical. Predecessor implementation may have landed since pack creation: **mark capability already delivered if backed by master code/tests, then move to the smallest missing acceptance criterion**. Do not reimplement a delivered feature. Your autonomy is over engineering choices *within* current doctrine; do not auto-accept proposed ADRs, remove prerequisite gates, or bypass authorization because they delay delivery.

### Checkpoint A: immediately usable, no new records
P00: open New Intent from existing IDD and prove route; P01: private autosave/revisions; P02: evidence-backed context; P03: reproducible prompt/pack preview and copy/export; P04: actual provider-assisted synthesis with honest unavailable fallback; P05: meaningful questions/answers. **Dogfood at each step.**

### Checkpoint B: authoritative planning + execution
P06: dedupe/impact classification; P07: shared draft validation; P08: explicit idempotent materialization; P09: intent to canonical issue execution; P10: full correctness and resilience gates; P11: owner dogfood + release proof, documentation and continuous improvement.

## Operator behavior
1. Check master SHA, peers/claims and relevant plans/PRs; derive a collision-free micro-slice.
2. Run one prompt. Land the slice; prove actual runtime behavior with test/evidence.
3. Re-measure: if feature already exists, use that prompt to close its actual next gap.
4. Stop feature fan-out whenever CI or landing queue is blocked; fix the delivery bottleneck with existing mechanisms rather than accumulating stale PRs.
5. Record status under the canonical issue/provenance trail, NOT a new markdown progress registry.

## Overall done definition
A real user can open `/ui/idd`, start a new draft, type and resume it; review traceable suggestions with real source links and freshness; ask/answer critical questions; inspect deterministic prompt and pack previews; see duplicates, covered and conflicting requirements; approve a guarded materialization diff; obtain valid canonical records; and run an issue using existing Majordomus execution, with evidence and derived intent verdict reflected in the same workspace. Tests use real fixtures and a real server, no fake backend/demo-only views.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.
