# MASTER MISSION: Integrate Prismatic Way Cognitive Fabric across Majordomus

You are Claude Code operating in `korczis/prismatic-majordomus`. Execute with maximum available reasoning and implementation effort. Your job is to **inspect, implement, migrate, test, enforce, document, and deliver** a repo-native capability, not just produce an architecture essay.

## Objective
Make existing Majordomus mechanisms continuously correlate user intent, repo knowledge, evidence, tasks, peer reasoning, and workflow outcomes through lifecycle events. Optimize for verified completed intents per unit latency, human effort, and compute cost. Use one canonical identity/provenance graph and existing state ownership. Avoid parallel orchestrators and manual cross-surface registries.

The neutral name of the discipline is **Prismatic Way**; the cross-cutting capability is **Prismatic Cognitive Fabric**. Use those terms in human-facing docs only where useful. Do **not** rename the product, establish a new service named `cognitive-fabric`, or create another framework merely because the prompt has a name.

## Mandatory repo preflight (do before implementation)
1. Read `AGENTS.md`, root `README.md`, `.ai/README.md`, `.ai/manifest.yaml` (discover actual path; never assume missing files), effective rules, profiles, workflows and relevant ADRs. Resolve scoped context before editing under each directory. `.ai/local` is private checkout state, NEVER normative, committed or published.
2. Read repository documents `docs/REASONING.md`, `docs/MESH.md`, `docs/MONITOR.md`, `docs/CAPABILITIES.md`, task lifecycle, evolve skill, intent and knowledge documentation, capability declarations and existing UI publication contracts. Do not treat any cited location as authoritative without checking current HEAD.
3. Run available equivalents of `majordomus context`, `majordomus worktree`, `majordomus reasoning advisors`, `majordomus knowledge candidates`, `majordomus knowledge check`, `majordomus intent ...`, mesh/peer inspection, and baseline quality checks. Command spellings must be verified via CLI help before execution.
4. Check active peer claims, GitHub PRs, branches, existing issues and concurrent work. Run `scripts/collision-check --new <candidate paths>` after determining candidate paths. Claim scope using existing mesh/task mechanisms; do not presume board announcements are exclusive locks.
5. Establish a **reuse matrix**: desired behavior → existing canonical source → transport → consumer → current test → actual gap. Identify relevant unfinished work and reuse it; no duplicate implementation.
6. Record exact starting commit, branch, uncommitted files, environment, baseline test results, baseline performance, existing working features, and blockers. Protect unrelated user changes.

## Operating contract
- **Discover once, model once, derive everywhere.** Canonical sources are typed `.ai/repo` declarations, existing runtime state, Git, event/mesh journals and authoritative evidence, by domain. Do not flatten distinct authorities into one invented database.
- **Events trigger; they do not decide.** Hooks are thin, authenticated, bounded adapters to existing capabilities. No substantive business logic or expensive provider calls in hot-path hooks.
- **Deterministic first.** Hashing, parsing, dependency traversal, schema validation, dedup and graph deltas precede LLM calls. Infer only where deterministic techniques cannot suffice.
- **LLM advice is not authority.** Use existing `reasoning` assessment / plan / consultation / record for material uncertainty. Independent advisors are optional and their agreement is not proof. Contradictions require evidence or experiment.
- **Zero consumer registration.** Add a canonical entity and let existing capability/entity generators project to CLI, HTTP/OpenAPI, MCP, Cockpit and site. Do not hand-edit transport registries or generated pages.
- **Private is private.** No raw prompts, transcripts, credentials, local knowledge, reasoning traces or per-user events in public GitHub Pages. `.ai/local` must never enter publicly served data.
- **No unbounded autonomy.** Follow existing worktree, task scope, mesh claim, authorization, test, CI, merge/release seats. Remote mesh events alone must never execute arbitrary code. Respect provider contracts and subscription/API boundaries.
- **Fail honestly.** Keep missing advisors, offline nodes, unknown evidence and unavailable backends explicit. Do not manufacture 'complete', 'verified', or 'live' from cached guesses.
- **Converge.** Idempotency and causal dedup prevent hook feedback loops. Reject or quarantine inconsistent graph mutations. Preserve auditability and rollback.

## Delivery plan
Execute the prompt files in this sequence, using each as an instruction module:
1. `01_DISCOVERY.md`: audit/reuse matrix, baseline and narrow integration design.
2. `02_EVENTS_INTENTS.md`: one end-to-end vertical slice event → existing intent/task identity → validated state mutation → projection. Tests first for invariants.
3. `03_KNOWLEDGE.md`: deterministic observation/candidate pipeline, invoke existing evolve/review, no unreviewed promotion.
4. `04_REASONING_MESH.md`: bounded/adaptive reasoning with optional peer review, trust/claim semantics preserved.
5. `05_SURFACES.md`: canonical read model, live projection, mobile UI, accessible controls, provenance and freshness.
6. `06_ENFORCEMENT.md`: schema, properties, concurrency, security, drift, performance and CI gates.
7. `07_DELIVERY.md`: dogfooding, release evidence, changelog/docs, PR landing handoff, final report.

Each wave must close a working vertical outcome before broadening. Parallelize **only independent scopes** after claiming them, with a clear integration owner. Stop feature expansion when a wave's gates fail and repair root causes. Do not block indefinitely awaiting advisors or user replies; follow task outcome/handover rules for genuine blockers.

## Minimum feature scenarios
A. Prompt describes existing intent: link, do not duplicate. B. Prompt introduces a genuinely new goal: create candidate, validate, only commit under policy. C. Relevant source changes: update impacted edges/evidence only. D. Completed test: attach evidence, not automatic truth of unrelated claims. E. Episode closes: create knowledge candidates using existing mechanisms. F. Candidate duplicated/contradicted: reject or defer with evidence. G. Peer review unavailable: continue local-first. H. Two nodes race: idempotent reconcile and conflict visible, never silent overwrite. I. Client reconnect and replay: identical result. J. UI goes stale/offline: clear timestamps and states; no fake live data. K. CI does not leak `.ai/local`/secrets.

## Acceptance
Meet every invariant and scenario in `contracts/ARCHITECTURE.md` and `verification/ACCEPTANCE.md`; produce `verification/REPORT.md` populated with actual evidence. Run focused tests, cross-surface tests, full relevant gates and publication smoke tests. Benchmark against baseline, not arbitrary slogans. Document backward compatibility and migrations. Report unimplemented parts explicitly. Do not claim released until merge/tag/deploy are verifiably observed.

## Final response
Provide: baseline vs after; architectural ownership; files/PRs/commits; specific commands and outcomes; tests; measurements (token/cost/latency when measured); cross-surface proof; unresolved debt; exactly which gates remain. No theatrical claims of perpetual autonomy or 'forever working'.
