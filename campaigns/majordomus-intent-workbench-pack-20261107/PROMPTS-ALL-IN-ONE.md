# All-in-one | Majordomus Live Intent Workbench prompt pack

Use each Pxx section as a separate Claude Code turn. Do NOT blindly execute all as one mega-task.



---

<!-- SOURCE: 01-MASTER-ORCHESTRATOR.md -->

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


---

<!-- SOURCE: 02-BASELINE-AND-COLLISION-MAP.md -->

# Baseline and verified sources (snapshot, NOT future truth)

As inspected on 2026-11-07, GitHub `master`: `d7fd9a3617024a7390ac36c1888d6fcee63b600e`. Recheck current master, issues and PRs before work.

## Canonical code/docs
- GitHub repo: https://github.com/korczis/prismatic-majordomus
- IDD workspace: https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/ui/idd.rs
- Capability `intents`: https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/capability/builtin/intents.rs
- Plan capability: https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/capability/builtin/plan.rs
- Source context: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/README.md
- Docs: https://github.com/korczis/prismatic-majordomus/blob/master/docs/CAPABILITIES.md
- Planning: https://github.com/korczis/prismatic-majordomus/blob/master/docs/PLANNING.md
- Reasoning: https://github.com/korczis/prismatic-majordomus/blob/master/docs/REASONING.md
- Owners' pack policy: https://github.com/korczis/prismatic-majordomus/blob/master/docs/OWNER_PACKS.md
- Proceed: https://github.com/korczis/prismatic-majordomus/blob/master/docs/PROCEED.md

## Existing intent contracts
- `the-owner-directs-work-from-the-ui`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/the-owner-directs-work-from-the-ui.yaml
- `a-brief-is-reconciled-into-the-plan`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/a-brief-is-reconciled-into-the-plan.yaml
- `what-a-workspace-decides-reaches-the-repository-with-its-source`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
- `intent-planning`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/intent-planning.yaml

## Existing issue dependency graph, do NOT replicate
- I3251 #1588: brief grounded in existing project evidence.
- I3252 #1589: in-memory `intents.draft_check` + `intents.draft_write` checked canonical write; open, derived BLOCKED at inspection; depends on I3251 and I3247.
- I3254 #1591: duplicate/overlap detection, open, derived BLOCKED; depends on I3251.
- I3272 #1604: campaign revision/source digest; open, BLOCKED; proposed ADR dependency.
- I3273 #1605: content-derived requirement identity/reconciliation; open, BLOCKED; depends on I3272, I3252, I3251, I3254, I3280, I3281, I3278.
- I3274 #1606: changed requirement segment diff; BLOCKED.
- I3275 #1607: campaign provenance in intent inspectors; BLOCKED.
- I3281 #1612 through I3286 #1617: workspace reference and remote access; several BLOCKED, significant auth/provenance policy prerequisites.
- I4484 #2872: display intent gap/critique in UI; open, BLOCKED.

Important: Issues' `open` state on GitHub is NOT authoritative plan status; Majordomus derives READY/BLOCKED/VERIFY/DONE from canonical records and evidence. These observations are hints for collision avoidance, not a license to overwrite dependency ordering.

## Critically different concepts
1. **Draft (private)** is mutable working state under an existing private checkout/workspace store, not a repository intent or a plan status.
2. **Intent (canonical)** is a typed tracked record whose stage/verdict are derived.
3. **Campaign** is preserved external source/provenance, not an alias for raw chat logs or AI output.
4. **Outgoing prompt pack** is a derived instruction artifact; the current `pack` capability inspected on master shards the repository into bounded text for retrieval. Do not conflate them.
5. **Reasoning** records evidence-based engineering decisions; a model recommendation does not authorize writing a plan.
6. **GitHub Pages** is a published static projection; interactive draft/writes require a trusted runtime.

## Usage note
This pack is source-grounded but cannot know which work has landed after its snapshot; therefore each task explicitly requires current master, active scopes, tests, policy and dependencies to be rechecked before making changes.


---

<!-- SOURCE: 03-ACCEPTANCE-AND-GATES.md -->

# Workbench acceptance matrix and launch check

| Test scenario | First expected | Owning prompt |
|---|---|---|
| Open `New Intent` from `/ui/idd` | Real editor; no tracked writes | P00 |
| Edit long multilingual free text, refresh, resume | Exact draft and revision restored | P01 |
| Two tabs edit same revision | Conflict, never silent overwrite | P01 |
| Draft cites ADR + existing intent with valid links | Reasons + source IDs + freshness | P02 |
| Changed text while old request is in flight | Old result discarded | P02, P03 |
| Actual model analysis with evidence | Real source-backed suggestion, never fake | P04 |
| Provider missing | Explicit unavailable/partial; editor still works | P04 |
| Ambiguity requires decision | One relevant question, answer updates draft | P05 |
| Prompt and pack from one revision | Stable hashes, copy/export, source anchors | P03 |
| Requirement restates an intent or issue | Match existing, no new issue | P06 |
| Draft has uncovered criterion / cyclic dependency | Shared validator refuses without writing | P07 |
| Explicit approval of current exact digest | Valid canonical records, one write | P08 |
| Same approval key repeated | No duplicate records | P08 |
| Master/source revision changes before approval | Refused stale, recheck required | P08 |
| Ready issue dispatched twice | One existing execution, canonical status | P09 |
| Evidence fails | Cannot mark intent satisfied | P09 |
| Malicious HTML/prompt injection/private canary | No execution, no XSS, no public leak | P00–P10 |
| End-to-end on real server | Browser reaches canonical plan | P10 |
| Master/release/deploy evidence | Observed, not inferred from PR green | P11 |

## Minimum launch checkpoints

**Checkpoint A: LOCAL FIRST USABLE**
- A real server route and New Intent entry exists.
- Draft survives reload.
- Context cards have real source anchors and state.
- Prompt and atomic pack are copyable, revision-bound, and contain truthful citations.
- Assistant answer actually executes when available; fallback is explicitly unavailable.
- No raw transcript, credentials or private draft in committed/public artifacts.

**Checkpoint B: MATERIALIZED + EXECUTABLE**
- Dedupe + coverage classification grounded in canonical records.
- In-memory validator identical to actual plan/intent validation.
- Materialization is human-authorized, CAS-safe, idempotent and atomic.
- Issues are canonical and reach existing proceed/execution lifecycle.
- Derived satisfaction remains evidence-gated; published/static UI is not the local editor.

## Delivery gates per prompt
1. Current master SHA and repo context resolved.
2. Existing issues/PR collisions and dependencies considered; scope/peer claim obeyed.
3. Rust/Cargo tests for touched area; browser test if UI changes.
4. Relevant `majordomus intent validate`, `majordomus plan validate` and `scripts/ui check`, using actual supported invocation.
5. `majordomus generate` and `majordomus generate --check` for generated surfaces when relevant; use-case impact/coverage where relevant.
6. Security/privacy tests for private state and public projection if touched.
7. Canonical docs, version/release policy, diff review, no placeholder artifacts.
8. Pushed PR + observed CI + guarded merge to master if authorized, otherwise explicit evidence-backed blocked state.
9. Real runtime reproduction, and deployed surface verification when genuinely applicable.


---

<!-- SOURCE: prompts/00-P00-LAND-THE-ENTRYPOINT.md -->

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


---

<!-- SOURCE: prompts/01-P01-PRIVATE-DRAFT-AND-REVISION.md -->

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


---

<!-- SOURCE: prompts/02-P02-EVIDENCE-BACKED-CONTEXT.md -->

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


---

<!-- SOURCE: prompts/03-P03-LIVE-PROMPT-AND-PACK-PREVIEW.md -->

# P03: Live Prompt And Pack Preview

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
A user can **immediately use** the Workbench as a prompt workbench: a source-grounded evolving prompt AND atomic prompt-pack preview can be copied/exported. The preview is derived from the current draft, decisions and source digests, with explicit staleness. It neither executes nor writes plan records.

## Implement one vertical
- Compile from one typed synthesis projection: goal, verified existing behavior, invariants, exclusions, assumptions, questions, acceptance criteria, validation, provenance, and exact scope. Reuse existing prompt conventions and pack/campaign source policy. `pack.plan` in current Rust capability directory represents token-bounded repository shards; do not silently reuse its name/semantics for user prompt packs.
- Stable IDs/order and digest for unchanged requirements; source links are real; missing evidence yields explicit `unverified`, not fake citations. Avoid cost-heavy full recompile when only a small segment changes.
- Offer compact prompt, full implementation prompt and atomic work items, all as views of identical semantics. One copy action per format and export of a safe UTF-8 artifact if existing UI supports downloads. No prompt file enters the tracked tree without explicit future promotion.
- Each atomic prompt describes one acceptance-checkable vertical slice, reuses canonical source, respects claims/CI/landed master, and has a test and delivery contract. Never output fake PR URLs, issue numbers, version bumps or deployment proof.
- Render all versions with safe escaping. Tests: deterministic fixtures, stable digests across order reshuffling, single requirement edit affects only relevant part, stale revisions blocked, no source leak, copy/download content parity.

## Proof / next
From a saved editor draft show a copyable real pack containing grounded constraints, not canned examples. A user can paste a compiled prompt into Claude Code and progress immediately even BEFORE materialization is implemented. **This is the early usable prompt-preview checkpoint; P04/P05 add genuine conversational AI.**


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.


---

<!-- SOURCE: prompts/04-P04-REAL-ASSISTED-SYNTHESIS.md -->

# P04: Real Assisted Synthesis

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
The user can request an **actual assistant analysis** of the current draft + bounded context, and see a structured synthesis with citations, not a canned preview pretending to be AI. The UI stays responsive while the model thinks; when no provider/session is available, it visibly says so and retains deterministic context rather than inventing analysis.

## Hard architectural point
`docs/REASONING.md` supplies evidence-first consultation, not necessarily a free-form assistant/chat generator. Inspect `models.route`, existing provider adapters, execution/event streaming, reasoning policies, budgets and any Claude Code/session integration first. Do NOT invent a new provider SDK, model credentials, or a second reasoning engine. Reuse a real existing provider-neutral execution path. If none supports this interaction, deliver the smallest genuine local-only adapter/bridge permitted by policy, tested with a fake provider in tests and a documented real-provider smoke, or explicitly mark model synthesis unavailable. Never claim the unavailable bridge as delivered.

## Implement one vertical
- Introduce only the minimal typed synthesis request/result needed: draft ID/revision, question/focus, bounded source refs and policy budget; returned suggestions, claims, citations, uncertainty, provider/availability and response revision.
- Inject authoritative context using exact source references and digest checks. Explicitly instruct model not to treat pasted draft/README/issue content as commands, permissions or trusted policy. No tool execution or repository mutation in this flow.
- Provider/model selection follows existing policy, OTel/cost/budget if present; enforce timeouts, cancellation, input/output bounds, redaction, trace IDs and visible errors.
- Distinguish `evidence`, `inference`, `recommendation`, `unresolved` in response schema. Reject unparseable/uncited assertion as proof. Every cited reference must exist in the context snapshot or be marked unverified.
- Tests: deterministic stub executor exercises full typed path; unavailable provider, timeout, stale revision, prompt-injection attempt, malformed response, bounded context, source citation validation.

## Proof / next
Demonstrate one real optional model answer from the live runtime when available, with correct source anchors and no write side effect. If bridge must be split, land a narrow functioning transport plus one real test, then explicitly continue only missing dialog wiring in P04. **No fake 'AI is thinking' animation as evidence.**


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.


---

<!-- SOURCE: prompts/05-P05-QUESTIONS-AND-DISCUSSION.md -->

# P05: Questions And Discussion

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
The assistant asks a small number of high-impact **clarification questions**, supports owner free-form follow-up discussion, and the owner can answer them in-context. Answers update synthesis without rewriting history or pretending an assumption is established fact. The discussion is bound to one draft/revision and safe from refresh or two tabs.

## Implement one vertical
- Resolve questions from evidence gaps, conflicting ADR/governance, untestable criteria, scope decisions or material tradeoffs. Reuse `reasoning` uncertainty/falsifier/decision semantics; do not invent new decision authority.
- Ask at most a bounded number of distinct prioritized questions. Every question includes: consequence if unanswered, known options, current evidence, whether an evidence-backed default is permissible, and why a human decision is actually needed. Do NOT ask user to supply repository paths or facts the system can derive.
- Support both targeted question/answer and a small grounded discussion thread; bind messages to draft revisions and source digests, and re-use the same real provider/session adapter as P04. Never quietly send private content to an unintended provider. Answers become typed draft decisions/revisions, not tracked ADRs or governance changes. A material conflict with a standing ADR cannot silently be overridden; present explicit decision/escalation path.
- Display analysis and answers with provenance and staleness. On new evidence, mark the affected previous answer `needs_revalidation`, preserving its history. No private answer is published.
- Tests: repeated synthesis deduplicates questions, answer changes downstream preview, no-governance bypass, distinct concurrent revisions conflict, no new questions on an identical snapshot, provider offline still shows evidence-backed open questions if they can be deterministically derived.

## Proof / next
Dogfood: type an ambiguous performance intent, answer whether latency is p95 runtime or build latency, observe criterion and prompt preview changed with the decision. Land one real end-to-end question/answer path, then P03 preview updates.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.


---

<!-- SOURCE: prompts/06-P06-RECONCILE-DUPLICATES-IMPACT.md -->

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


---

<!-- SOURCE: prompts/07-P07-DRAFT-CHECK-SAME-VALIDATORS.md -->

# P07: Draft Check Same Validators

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
The Workbench can preview a proposed canonical intent + milestones/issues and get a **real validation verdict as if the draft were written**, without altering the tracked tree. The result is a typed per-criterion diagnostic with provenance, dependencies, coverage, conflict and freshness.

## Existing work
Treat I3252 (#1589) as the primary canonical delivery lane. Its accepted scope describes `intents.draft_check` and `intents.draft_write`; inspect live status and I3251/I3247 dependencies. Do not build a second validator, temporary copy of business rules, or new arbitrary status model.

## Implement one vertical
- Normalize proposed records using canonical current Rust intent/plan schemas and validators. Prefer an in-memory overlay against the same index/plan validation pipeline, not a mutated working tree.
- Return exact blockers, warnings, source digests, failed prerequisites, uncovered criteria, cycles and invalid refs. Display check result on the same draft with a clear `current/stale` fingerprint.
- Same check must be reachable from all canonical capability projections where appropriate, with typed input and deterministic output. Do not create write endpoint yet if it cannot be made safe within this slice.
- A draft from a different master/repo revision is never treated as previously validated; source/fingerprint drift invalidates validation.
- Tests: dangling dependency, uncovered criterion, invalid evidence, missing milestone, valid fixture, check never writes even on failure, CLI/MCP/HTTP parity where exposed, no extra schema not read by validator.

## Proof / next
Demonstrate `draft_check` declining an invalid fixture and accepting a valid one, with `git status` unchanged. Integrate canonical issue I3252 implementation as far as this slice completes; do not label entire issue `DONE` until checked write is proven in P08.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.


---

<!-- SOURCE: prompts/08-P08-EXPLICIT-MATERIALIZATION.md -->

# P08: Explicit Materialization

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
A human reviews a **diff of exact canonical plan records**, explicitly authorizes promotion, and the server commits the checked proposal through the repository's existing guarded writer/claims. It is atomic, idempotent, CAS-protected and reflected by `intents.list`, `plan.*`, `/ui/idd`. No draft autosave or AI answer can invoke it.

## Preflight/authority
Recheck I3252, I3247, I3254 and campaign/provenance dependencies. Some upstream ADRs (e.g. campaigns/remote workspaces) are explicitly *proposed*, not authorization. Never auto-accept them. For a pure first-party local draft, prefer the narrowest existing `intents.propose` path and valid checked-write extension; do not claim full campaign/workspace promotion until their dependencies land. Respect server write permissions, CSRF/origin, branch/worktree, scope/ID claims and governance policy.

## Implement one vertical
- Present proposed record paths and structured before/after content, canonical IDs, source references, acceptance criteria, dependencies, inferred `serves` relationships, staleness and outstanding review decisions. Provide a **dry-run preview** with no mutation.
- Write ONLY when an explicit human action submits the exact approved digest/revision. Re-run checks, duplicate classification and current HEAD/source digests under write lock; refuse stale or conflict without partial files.
- Use atomic write/rollback transaction and repo's canonical ID allocation, not timestamp/random IDs for tracked records; idempotency key repeats return the prior result. Generated GitHub issue sync is separate and uses existing `scripts/github-sync`/guarded workflow, not a surprise network side effect.
- Do not copy raw conversation into tracked files. Track typed, valid provenance pointers only where grammar and public privacy gates already admit them. If provenance is not yet accepted, refuse that promotion mode explicitly while allowing an authorized local-source path defined by current schema.
- Tests: unauthorized request, same key twice, stale digest, two-tab conflict, invalid proposal zero writes, interrupt/retry partial-write rollback, generated records pass `intent validate`, `plan validate`, and UI auto-discovers new records.

## Proof / next
Use a disposable repository fixture to prove request → validate → approve → write → explain, and a real browser approval flow. Land guarded write, then P09 connects execution. Do NOT bypass if a required dependency/ADR still blocks it.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.


---

<!-- SOURCE: prompts/09-P09-EXISTING-EXECUTION-AND-FEEDBACK.md -->

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


---

<!-- SOURCE: prompts/10-P10-SECURITY-PERF-E2E-GATES.md -->

# P10: Security Perf E2E Gates

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
A real server/browser end-to-end suite proves the entire currently implemented Workbench loop and makes accidental regression hard. Focus on high-risk failure modes instead of creating another documentation-only quality program.

## Implement one vertical
- Add a disposable fixture repository and browser test building on the existing UI, intent, planning, campaign and execution cases. Drive New Intent → edit → refresh → source citation → assistant response with deterministic provider stub + optional real-provider smoke → answer question → compile prompt → classify duplicate → dry-run check → authorized materialize → canonical issue. Follow execution only if P09 is verified.
- Run tests against actual HTTP server routes, not mocked controller HTML. Check typed capabilities via the actual projected protocol where appropriate. No snapshots as truth without semantic assertions.
- Attack tests: malicious pasted Markdown/HTML, prompt-injection in source content, CSRF/origin bypass, local-only private content via remote/public interface, static site canary leakage, stale/cancelled model response, race between approval and changed master, interrupted write, duplicate idempotency key, unavailable model/forge, malicious archive or untrusted campaign.
- Time and measure important spans: editor input p95 goal <100ms, local UI rendering not waiting on LLM, read/citation retrieval cost, token budget vs useful evidence; collect observed metrics, not invented performance scores. Use existing observability/quality metrics, no parallel telemetry stack.
- Ensure `majordomus generate --check`, `scripts/ui check` and relevant CI quality gates genuinely enforce the new contract. Update actual requirement → test → evidence link in existing plan. Correct source-order bugs at the owner, not by creating five frontend sorts.

## Proof / next
Provide exact test/run URLs, commit SHA and observed results. Leave optional unsupported integrations marked unavailable, not fake green. Land one coherent hardening PR within the timebox; if E2E breadth exceeds the slice, prioritize the highest risk, then do next prompt against the remaining evidence gaps.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.


---

<!-- SOURCE: prompts/11-P11-OWNER-DOGFOOD-RELEASE.md -->

# P11: Owner Dogfood Release

**Invocation:** paste this entire file as one Claude Code task. **One real slice, <=30 minutes target; master-first delivery.**

## Vertical outcome
On master, the owner can start using the Intent Workbench today with a clear entrypoint, truthful operating instructions, reliable integration and a verified release/deploy path. Remaining dependencies are visible as *specific unmet criteria*, not generic 'work in progress'.

## Implement one vertical
- Dogfood one real owner intent about the Workbench itself (test content safe to publish only if explicitly authorized; otherwise use a synthetic intent fixture). Prove: open route, type, refresh, analyze grounded sources, ask/answer, compile and copy an implementable prompt pack. If P08/P09 landed and policy permits, prove one guarded plan write + execution handoff with fresh evidence. Do not create redundant plan records.
- Audit UI UX: first paint, keyboard, responsive widths, long draft, Unicode/Czech/English, empty/error/offline states, stale revision badges, accessible citations, clear status of unpublished vs tracked, no untrusted HTML. Use the existing multilingual and design-system architecture; do not duplicate manually maintained translations.
- Update `docs/COCKPIT.md`, `docs/PLANNING.md`, `docs/OWNER_PACKS.md`, `docs/REASONING.md`, route help, generated docs/README only to reflect what is implemented. Add concise 'first 60 seconds' usage in the user-facing UI/help. Ensure GitHub Pages docs/landing projections are generated correctly without claiming the static site can edit local state.
- Verify correct canonical versioning and release record for merged work according to policy; observe actual master commit, pages/deploy build pointer and runtime (if targeted by policy). Document exactly which surfaces are deployed and which require running a local authenticated server. Do not hardcode URLs if derived.
- Produce a compact delivery ledger **in your final report**, not a new tracked registry: capabilities DONE/NOT DONE, linked existing issue status, p95 measurements, reproducible commands, route, latest landed master SHA, release/deploy evidence, and top 3 next bottlenecks.

## Proof / next
Owner should be able to launch a server following repo docs and use the product without further Claude prompts to create the editor itself. Remaining remote workspace/chat connectors, campaign ADR acceptance and advanced optimization are explicitly scoped and must not block local-first MVP.


## Mandatory Majordomus delivery contract (read for EVERY slice)

Work in `korczis/prismatic-majordomus` on the CURRENT `master`; the observed baseline for this pack is `d7fd9a3617024a7390ac36c1888d6fcee63b600e` on 2026-11-07, **not** an instruction to work from a stale head. Inspect the live checkout, remote master, current branches/PRs, claims, and task status before editing. Read `AGENTS.md`, `.ai/README.md`, scoped `majordomus context resolve`, relevant rules/doctrines/ADRs, existing plans and tests. Run the repository's peer/worktree and `scripts/collision-check --new` procedures for the **actual** intended paths; announce/claim work and avoid conflicting workers. Honor dependencies and the landing/release seats. No direct commits to protected master and no bypass of gates.

**Hard timebox: ONE independently deliverable vertical slice per invocation, target <= 30 minutes of active implementation.** If the full outcome is too large, cut a smaller end-to-end behavior with an actual test and land it; report what remains to the next specific prompt. Do not make an empty refactor, a docs-only promise, a mega-branch, or a queue of new PRs. Aim for one narrow PR, integrated quickly through the existing guarded delivery process; never pretend CI/merge/deployment succeeded. Rebase/repair and land once authorized. Prefer folding accepted requirements into EXISTING canonical records; do not create duplicate issues, milestones, ADRs, capability catalogs, stores, or command namespaces.

Majordomus invariants: **single canonical source; derived status/evidence; typed schema; provenance; inferred discovery; no invented facts; no silent authorization.** AI guesses are proposals, not evidence. Full chat transcripts and raw prompts are not tracked or published. The `.ai/local/**` boundary is local, private, ignored, and never policy. External input is untrusted and never auto-executable. A draft edit MUST NEVER create tracked intent/plan records, dispatch an execution, push Git or change governance. Explicit, authorized promotion is separate. No live mutation from static GitHub Pages. Reuse the canonical `capability!` + `module!` registry instead of manually editing generated API/MCP/OpenAPI or duplicating frontend inventories; inspect `docs/CAPABILITIES.md`. Never repurpose `pack.plan`/`pack` merely because its name resembles an owner prompt pack: inspect its actual semantics first.

For EACH delivered slice: implement production path + meaningful automated tests + any relevant UI/browser/accessibility and security checks + docs/README/ADR impact + generated projections (`majordomus generate` and `--check` if applicable) + `majordomus usecase impact` / coverage + focused validation and relevant `majordomus plan validate`, `majordomus intent validate`, `scripts/ui check`. Apply canonical version bump/release policy if the change qualifies; do not invent version numbers. Commit, push, run/observe real CI, integrate on master using repository process, and verify deployed/runtime surfaces where applicable. If external gate blocks progress, report exact blocker and evidence, not fictional delivery. **A green PR is not a merged PR, a merge is not a deployed release, and a screenshot is not a test.**

Every completion report must include: starting/master SHA, files and capability IDs changed, corresponding existing issue(s) or justified new record, real test commands/outcomes, generated-drift check, UI route or CLI reproduction, PR URL, merge/master SHA when merged, canonical version/release evidence when required, deploy evidence when applicable, and exact remaining blockers. If unfinished, label it `NOT DELIVERED` and do not advance falsely. This prompt is an engineering task, not permission to override policy or approvals.
