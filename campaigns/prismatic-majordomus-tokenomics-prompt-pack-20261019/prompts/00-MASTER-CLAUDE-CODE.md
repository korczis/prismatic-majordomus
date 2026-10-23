# MASTER MISSION: Evidence-Driven Tokenomics Across Prismatic Majordomus

You are an experienced Rust / AI observability / distributed systems / product engineering team working as one Claude Code worker within `korczis/prismatic-majordomus`.

Implement, integrate, test, document, derive, enforce and demonstrate **real, provider-neutral token economics**, from usage telemetry to accepted outcomes, across Majordomus' existing IDD/IID work lifecycle, mesh, canonical `.ai/`, Cockpit, `/ui`, CLI, API, MCP, OpenAPI, GitHub Pages, docs and generated surfaces. No mocks, stubs, static placeholder graphs or self-reported claims presented as observations. Do NOT develop another AI agent framework, workflow engine, event bus, registry, accounting database or dashboard framework.

## NON-NEGOTIABLE REPOSITORY PREFLIGHT

- Inspect current default `master` and current branch/head. Read `README.md`, `.ai/README.md`, `.ai/manifest.yaml`, effective rules with dependencies, scoped READMEs, architecture/ADR/workflow docs, AGENTS.md, CLAUDE.md, issues and existing integration implementations. Do not assume today's structure from this prompt.
- Run `majordomus context`, `majordomus worktree`, board/peer inventory, relevant `majordomus context resolve <path>`, `majordomus usecase impact`, collision checks and mesh announcement / exclusive claim per repository policy. Observe canonical worktree topology, landing/release seats and PR workflow. Never race another worker or land from worker seat.
- Inspect and address existing GitHub issues `#364`, `#365`, `#366`, `#367`, `#368`, `#369`, `#372` and related issues before creating any; inspect requirements, closure evidence, dependency graph and the actual implementation. Reuse existing task/plan/issue IDs, or make new issues only for genuinely absent requirements.
- Inspect the current meaning of IDD/IID and its actual graph/entity model. Reuse intent/issue/milestone/wave/outcome/execution semantics; do NOT silently invent alternate IDs or change terms. Explicitly map synonyms in documentation only if verified.
- Find existing telemetry / execution records / history / event ingest / monitor / pubsub / mesh / storage / UI route contracts before modifying. Produce a compact evidence-based reuse/extend/delete decision table.
- Use existing `capability!` / canonical module declarations for CLI/API/MCP/OpenAPI surfaces, then `majordomus generate`. No hand-authored parallel endpoint/Swagger inventories.
- Resolve material uncertainty with `majordomus reasoning` and bounded experiments, as prescribed by current repository rules. Distinguish source fact from architectural proposal.
- Deliver vertical slices and ongoing durable commits/PR evidence through current release machinery. Do not skip gates or falsely close issues to increase throughput.

## DOCTRINE: SERIOUS ECONOMICS, NOT COSMETIC METRICS

1. Metering facts: provider, emitter, event/request identity, operation, timestamp, token categories, original provider units, pricing basis, estimated/reconciled USD, storage provenance and source-version.
2. Identity and causality: correlate usage to a *verified* Majordomus execution/worker/session/intent/issue/milestone/outcome only with an explicit join rule. Preserve many-to-many ownership and unallocated costs. Never arbitrarily divide unattributed usage and call it ground truth.
3. Economic outcome: measure accepted, independently evidenced outputs (e.g. test-backed task completion, accepted/landed changes). Commits, LOC, PR count and agent assertions are activity proxies, NOT impact.
4. Three distinct labels: **observed** (reported by emitter), **derived** (deterministic projection), **inferred/estimated** (assumptions with method and bounds); **reconciled** must be separately marked and only when billing authority genuinely exists.
5. Never assert savings from cache tokens alone. Distinguish physical token usage, cache-billed rates, counterfactual gross-equivalent cost, actual invoice, and modelled savings. Attach a baseline and confidence interval to experiment-derived claims.
6. Costs on subscription plans do not equal marginal invoice charges. Support accounting basis `api_equivalent`, `reported_estimate`, `provider_invoice`, `subscription_allocation`, `local_compute_internal` etc. Never add them without explaining comparability.
7. Explicit missingness: silent source = `unobserved` with cause, period and coverage denominator; zero = measured zero. Instrumentation outage is not a cost decrease.
8. Security: metadata-first; never ingest raw prompt text, responses, tool content or secrets by default. Minimize identifiers and redact sensitive attributes. Local-only storage and authenticated remote access by default; static Pages must publish only safe aggregate/public projections with freshness and provenance.
9. Policy/governance belongs in `.ai/repo/`; raw telemetry must NOT be committed there, and `.ai/local/` may never become public/normative. Use existing canonical runtime persistence rather than creating a duplicate `.ai` data store.
10. Avoid AI calls to measure AI calls. Keep instrumentation out of hot paths; bounded batches, backpressure, idempotence, bounded overhead. Opt-out and fail-open for ancillary metrics, fail-closed only for explicit compliance/quality claims when the policy requires it.

## TARGET CAPABILITIES

**Collection**: Verify official Claude Code OTel / hooks / StatusLine interfaces against installed version. Enable telemetry via launcher environment / user-managed settings consistent with precedence. Implement collector/adapter only when no reusable supported receiver exists. Support other providers through shared typed adapter boundary *only to the extent observed source data permits*. If Codex/Gemini/local sources lack reliable counters, show coverage gaps, do not hallucinate token counts. Never MITM requests or parse transcripts by default. Handle retries, partial exports, cumulative vs delta metrics, counter resets, repeated deliveries, clock skew, cache categories, prices changing over time, currency, price tier, rate limits and billing freshness.

**Attribution**: Hook session/prompt/worker/tool IDs to execution records and IDD intent tree; DAG attribution should be transparent, reproducible, confidence-labelled. Model nested subagents, mesh peer hops, resume/compaction, crossover between tasks, grouped work, CI and unattended proceed/evolve workflows. Prefer reference edges to stored duplicated summaries. Maintain causality under clock skew and offline merge. Missing correlation produces unallocated bucket, not guessed issue.

**Metrics**: request/session/day/provider/model/agent/skill/tool/MCP/issue/milestone/intent/wave/outcome economics; spend rate, usage rate, cache read/write ratio, context churn, compaction effectiveness, tool overfetch, failed/retried work cost, cost per accepted outcome, cost per verification, time to accepted outcome, machine running costs vs API-equivalent costs, model routing candidates, budgets and confidence. Define denominators and grouping compatibility. Benchmarks / baseline / interval and opportunity estimates must be empirically testable with quality guardrails.

**Live UI**: Extend the canonical derived `/ui` and `/cockpit` surfaces, do not fork. Build responsive overview, per-provider/model stacked tokens, estimated-vs-reconciled, temporal spend/coverage, intent DAG cost overlay, mesh worker economics, agent/skill/tool contribution, cost-to-outcome distribution, failure/retry waste, budget burn, anomalies, cohort comparisons, evidence drilldowns and experiment A/B. Data must come from real API projections, not fixtures; use existing push/pubsub/WS/SSE infrastructure; reconnect/resume/replay and freshness metadata. Empty state must clearly say no measurement. Interactivity: timeline brush, attribution inspect, drilldown, filters, sorting, exports, dimensions, compare baseline, source/claim links and responsive mobile interactions. Accessible, documented, bounded queries, performant.

**Surfaces**: Add canonical capability projections into CLI, API/Swagger/OpenAPI, MCP, monitor stream, Cockpit and UI. GitHub Pages / landing should market only proven features with measured/live/static-snapshot semantics clearly labeled. No secrets, personal session data, invented historical token graphs, or client-accessible privileged endpoints. Keep links to canonical entities consistent with entity routing.

**Derived `.ai/`**: Add or extend minimal typed policy for budgets, telemetry permissions, retention, attribution rules, availability/coverage and optimized routing. Generate provider launcher settings, docs and tool metadata through existing generators. Add actual quality gates for schema and provenance, but never block productive work simply because a provider cannot export metrics unless a *declared* instrumentation guarantee is part of task acceptance. All enforcement claims need executable tests.

**Optimization closed loop**: observe → attribute → detect → propose → experiment → validate quality/non-regression → roll out / rollback → measure again. First introduce advisory recommendations. Automated changes must be policy-gated, auditable, reversible, with no hidden model downgrade, privacy regression or quality sacrifice. Give savings ranges, not invented exact dollar amounts.

## EXECUTION WAVES (INDEPENDENT PRS ONLY WHEN EXISTING LANDING SEAT ALLOWS)

W0 Discover & reconcile scope: evidence tables, canonical source map, issue inventory, profile current coverage. Identify blocker vs optional. No green claims without tests.
W1 Establish metering contract: canonical event + completeness/provenance and durable dedup/replay. Prove reporting an absent provider, not zero.
W2 Ingest Claude official OTel (and only needed semantic lifecycle hooks), verify real source; isolate compatibility/version behavior. Replay + duplicate tests.
W3 Join measurements into existing execution/intent/outcome graph with explicit, tested attribution algorithm, unallocated bucket, mesh-friendly causality.
W4 Economic algebra, price-version model, reconciliation states, quality-gated outcome measures, baseline/sensitivity/confidence. Coverage and missingness-first dashboards.
W5 Canonical capabilities/API/MCP/CLI + live monitored projections, no duplicated route semantics. Cross-surface contract tests.
W6 Ship `/ui` + `/cockpit` UX responsive graphs, interactions and verified streaming/reconnect. Evidence disclosure and privacy checks.
W7 `.ai/` policies, minimal enforceable rules, test use cases, documentation, Pages safe claims, opt-in optimization experiments and flywheel loop.
W8 Full dogfood: instrument real Majordomus work and at least OSCILLA/OSINT/ISIR where access exists and policy permits. If unavailable label blocked, don't fabricate data. Measure before/after across comparable accepted outcomes; demonstrate a verified optimization or state inconclusive.

For every wave use current task lifecycle: preflight → implement → focused tests → integration tests → docs → generated consistency → no secret leaks → CI check → review/land via seat → observed deployment → issue evidence. Parallelize only truly non-overlapping worktrees with mesh claims; do not duplicate shared schema edits concurrently.

## ACCEPTANCE / STOP CONDITIONS

- A single source of canonical economics entities and price/coverage projections; no duplicate “Claude dashboard” truth.
- Real OTel sample produces source-preserving counted input/output/cache tokens and labelled provider-estimated cost; the *same* replay produces exactly one accounted operation.
- Silent/unsupported/permission-denied providers surface explicit `unknown` and coverage percentages, never `0`.
- Unknown prices are unknown, subscription billed charge remains distinct from hypothetical API-equivalent spend.
- Cost joins to a verified execution/outcome through explainable lineage; ambiguous join remains unattributed. A merged PR doesn't count as successful work unless domain acceptance says so.
- No double counting when both metric+event streams report the same usage; reconcile distinct signal types, not sum blindly.
- UI and Cockpit both show backend-derived live updates and actual reconnect; no placeholder cards. CI/UI tests include accessible keyboard/mobile and empty/stale states.
- `.ai/` schema, governance, generated provider adaptations, relevant use-case scenarios, rules and CI gates pass.
- Page claims verified, publication privacy allowlist enforced, no raw source payload in public bundle.
- End-to-end user journey: launch instrumented Claude → execute bound IDD intent via mesh → ingest → cost attributed with provenance → verified outcome → drilldown in Cockpit → Pages safe aggregate → optimization experiment → measured comparison.
- Efficiency improvement must have quality non-inferiority criterion and confidence ranges; otherwise it is a hypothesis.
- Present final report: exact master SHA, sources, existing issues reused/closed, architecture reuse, file changes, executed commands and pass/fail, actual observed usage, cost basis, baseline vs treatment, confidence, UI URLs/screenshots, CI/PR/deploy evidence, privacy review, remaining known gaps.

Do not stop after writing design docs. Implement vertical slices, execute tests, verify deployment. Do not claim completion where authentication, telemetry, real provider access or deployment is unavailable; mark blocked with reproduction steps and close only evidence-backed requirements.
