# P02: Evidence Backed Context

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
A saved or in-progress draft displays genuinely relevant, clickable context from **at least two** existing canonical source classes (e.g., context/ADR + intent/issue, or git + rule), not guessed IDs. Results say WHY they match and which revision they came from. A failed source appears as incomplete coverage, not as no evidence.

## Inspect first
Read `majordomus context` and `context resolve` behavior, `docs/CONTEXT.md`, `docs/KNOWLEDGE.md`, canonical object/search, graph, `intents.list`, `plan.issues`, `reasoning` and any existing evidence/source type. Find an existing source or citation/provenance projection before designing one. Public repo sources and local private sources have different permissions.

## Implement one vertical
- Build a bounded read-only context query against the existing index/graph and plan, keyed by `draft_revision`, source fingerprints and allowed workspace scope; do NOT automatically load all `.ai/**` into an LLM.
- Return a small deterministic ranking with canonical URI, title, source class, link, digest/ref, reason and `read|partial|unavailable` coverage. Stable tie-breaker by canonical ID.
- Initiate requests on meaningful text changes and debounce/cancel superseded requests; ignore results of old draft revisions. Reuse cache where safe, invalidate by git/index source changes rather than stale forever.
- Render first useful source cards without blocking editor input; link to entity/ADR/issue evidence. Do not fabricate link URLs; use actual canonical routable addresses.
- Tests: fixed fixture with two relevant sources, determinism under shuffled results, stale response cannot win, empty/offline source coverage, no source escape/local transcript leak.

## Proof / next
Use a real draft mentioning a known canonical intent and ADR; prove returned IDs exist and links resolve. Deliver a measured context path on master. Deep historical correlation and broad external connectors can remain future enhancement, but current output must be truthful and useful.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.
