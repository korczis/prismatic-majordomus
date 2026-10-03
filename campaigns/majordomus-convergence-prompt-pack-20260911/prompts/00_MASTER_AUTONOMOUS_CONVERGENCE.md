# MASTER PROMPT — Majordomus Zero-Debt Convergence

You are operating inside the `prismatic-majordomus` repository with a large context window. Your job is not to propose a plan and stop. Your job is to finish the core architectural convergence, land it, and prove it.

Read first, in this order where they exist:

1. root and nested `AGENTS.md`, `CLAUDE.md` or equivalent runtime instructions,
2. `.ai/**`, especially rules, doctrines, policies, manifest, session context and repository-specific baselines,
3. relevant ADRs and schemas,
4. `README`, `docs/DEVELOPMENT_RUNTIME.md`, `docs/DOCTRINE.md`, `docs/QUALITY.md`, `docs/COCKPIT.md`, `docs/MCP.md`, `docs/CONTEXT.md`, `docs/ENTRY.md`, `docs/CLAIMS.yaml`, plan/status docs,
5. canonical command/capability/provider registries,
6. CI workflows and local gate scripts,
7. tests proving the current architecture.

Also read the prompt-pack `AUDIT_FINDINGS.md`, `DEFINITION_OF_DONE.md`, and `INSTRUCTIONS.md`. Treat audit numbers as hypotheses to re-verify, not truth to preserve.

## Mission

Reach a state in which Majordomus can truthfully prove all of these core invariants:

1. Every public development mutation has one typed canonical runtime owner.
2. CLI/shell, HTTP, OpenAPI, MCP and Cockpit are adapters/projections, not independent business implementations.
3. Every blocking rule is machine-enforced, test-covered, documented and evidenced.
4. Every new/changed production path is 100% covered by changed-line and changed-branch policy where measurable, with behavioral tests for public contracts.
5. Canonical ordering is deterministic and no longer accumulating accepted presentation-order debt.
6. Session/context/capture lifecycle is provider-neutral in the canonical model, with explicit adapter capability/degradation for Claude Code, Codex, Gemini and other configured providers.
7. Cockpit can manage/execute relevant capabilities, not only list them.
8. Guaranteed claims are linked to executable obligations and current evidence.
9. Repository entry converges to the intended local runtime/session/context/peer state without moving domain logic into `.envrc`.
10. Generated docs/GitHub Pages, CLI, API, MCP and Cockpit reflect the same reality.
11. Core migration baselines/debts are zero, not merely ratcheted.
12. The landed commit is green in canonical local gates and CI; docs/deploy are verified where credentials/network permit.

## Non-negotiable process

Execute stages 01 through 15 from this pack internally, in order. Do not wait for human confirmation between obvious implementation steps. Maintain a temporary debt ledger and evidence report as you work. Use the repository's own issue/session/handover mechanisms if they are reliable enough; fix them first if they block truthful use.

For each stage:

```text
inspect → reproduce → root cause → canonical owner → implement
→ migrate/delete legacy → test → expose/project → enforce
→ document → run gates → inspect diff → commit when coherent
```

Do not move on while a stage has an unresolved core failure unless a later stage is explicitly responsible for that exact dependency.

## First action: truth baseline

Before edits, produce a machine-backed inventory of:

- current HEAD/branch/worktrees/status,
- local/remote divergence when network is available,
- all canonical gates and their current results,
- current capability registry, especially effect/mutation classification,
- public mutating shell/CLI commands and their canonical backing,
- project/vendored rules and their enforcement wiring,
- relevant baselines/ratchets,
- provider capture/lifecycle adapters,
- coverage policies,
- Cockpit navigation/route/capability projection sources,
- claim statuses and evidence wiring,
- generated artifact drift,
- stale PR/worktree/branch risk where inspectable.

Do not trust counts in this prompt; derive them.

## Core convergence: eliminate split-brain development semantics

The supplied audit found accepted backing debts around commands such as `adr`, `checkpoint`, `decision`, `evidence`, `finish`, `handover`, `init`, `migrate`, `plan`, `question`, `rules`, `session`, `start`, `update`, and `usecase`. Re-verify the list.

For every public development mutation:

- identify its domain object and invariants,
- implement/reuse a typed canonical command capability,
- define typed input/output and errors,
- make capability metadata the source for effect/permission/surface projection,
- move business logic out of shell/frontends into the canonical implementation,
- make legacy shell commands thin adapters or remove them according to compatibility policy,
- route API/MCP/Cockpit invocation through the same executor,
- add unit, integration, E2E and regression tests,
- prove cross-surface equivalence,
- update generated OpenAPI/docs automatically,
- remove corresponding migration debt.

The migration is not complete while a baseline says “accepted debt”.

## Governance convergence

Inventory all blocking project rules/doctrines/policies. Every blocking core invariant must have:

```text
canonical rule metadata
→ validator
→ canonical gate/doctor/check invocation
→ tests that prove pass/fail/bypass behavior
→ evidence
→ relevant CLI/API/MCP/Cockpit visibility/action
→ documentation
```

A rule that only tells an agent/reviewer what to do is not mechanically enforced. Either make it executable or classify it honestly. For the core quality/governance invariants, target machine enforcement.

Do not duplicate rule inventories. Extend the canonical schema and generators.

## Testing convergence

Do not simply change `90` to `100` and congratulate the repository.

Implement a policy that distinguishes:

- 100% changed/new production line coverage,
- 100% changed/new branch coverage where supported,
- behavioral obligations for every changed public API/capability/command/rule,
- bug regression tests tied to root cause,
- repository-wide historical floor that never decreases.

Make it work locally and in CI. Add fixtures/tests proving the coverage gate itself cannot be bypassed trivially. Preserve reasonable exclusions only when they are explicit, reviewed and schema/policy-backed.

## Determinism convergence

Fix `order-check` and then eliminate its presentation-order debt rather than updating it upward.

Classify sort sites:

- canonical presentation order → migrate to the central ordering abstraction,
- algorithmic/local internal order → explicitly justify in code/checker architecture,
- shell collation → pin deterministic locale or remove shell ownership.

Add permutation/property tests and cross-process tests. Ensure concurrent discovery cannot determine output order.

## Provider/session convergence

Create one typed provider lifecycle capability model. Discover provider adapters from canonical provider metadata or a canonical adapter registry. Implement equivalent lifecycle semantics for configured providers to the extent each provider exposes hooks/events.

At minimum:

- prompt capture,
- session start,
- session end,
- pre-compaction/checkpoint equivalent where available,
- provider session identity mapping,
- explicit capability flags/degraded status,
- tests with provider fixtures,
- one canonical session/context store and handover flow.

Do not fake parity for providers whose host cannot emit an event. Expose the limitation in doctor/Cockpit/docs and use the best supported fallback.

## Cockpit convergence

Treat Cockpit as a generated/control projection of the same canonical runtime.

Fix any enum/nav/routes/docs inventory drift. Where capabilities are safe/allowed to invoke, provide schema-driven forms/action controls through the canonical executor. Show validation, execution output, evidence and rule status. Do not add frontend-only business semantics.

A new capability should automatically appear in relevant Cockpit/API/MCP/Swagger/docs surfaces based on metadata, unless metadata intentionally suppresses a surface.

## Claim/evidence convergence

For every `guaranteed` claim, make the relation executable:

```text
claim → obligations → implementation IDs → tests/gates → latest result → commit/run identity
```

A path to a test file is not by itself current evidence. Generate or ingest current verification evidence through the existing model. Make stale evidence detectable.

## Entry/runtime convergence

Keep `.envrc` stupid. It may trigger Majordomus but must not duplicate repository-domain discovery.

Build/reuse a typed repository environment/runtime convergence subsystem with bounded latency. It should:

- find/attach to the correct local server or start it through the supported mechanism,
- detect stale/missing executable/runtime state,
- make repair/convergence explicit rather than silently failing,
- establish/refresh session/context/peer registration where supported,
- expose state via CLI/JSON/API/MCP/Cockpit,
- avoid network/build work in the fast banner/entry path unless an explicit asynchronous/maintenance path owns it,
- be tested for cold, warm, stale and failure scenarios.

## Git/worktree/landing convergence

Find unfinished work that belongs to the target architecture. Reconcile worktrees/branches instead of duplicating it. Do not blindly merge stale experiments.

Before final closure:

- canonical gates green,
- tests/coverage green,
- generated drift zero,
- docs/site synchronized,
- no required work stranded,
- clean git state,
- coherent commits pushed,
- remote CI verified,
- GitHub Pages/deployment verified where available.

If external credentials/network prevent push/deploy verification, finish every local obligation and report the exact external boundary. Do not claim deployment.

## Foundation gate before expansion

Do NOT spend significant effort on new distributed/LAN/routing/telemetry/auth features until the core convergence definition of done is green.

Only then, if the repository plan already contains distributed peer discovery/cooperation work and it is in scope, implement it through canonical provider/peer/capability/runtime models, with authentication/trust boundaries, discovery TTLs, deterministic identity, tests, Cockpit/API/MCP observability and docs.

## Adversarial closure

After implementation, try to break your own claims:

- add a fake blocking rule without a validator,
- add a new mutating capability,
- shuffle discovery order,
- create a second lease reader/parser,
- remove a provider lifecycle field,
- create nav/route mismatch,
- introduce uncovered changed production branch,
- alter a generated artifact manually,
- create a guaranteed claim with stale/no evidence,
- bypass a shell adapter directly.

The correct gates must fail. Revert fixtures after proving the guards.

## Final acceptance

Do not finish until `DEFINITION_OF_DONE.md` is satisfied or a genuinely external blocker is explicitly proven.

Produce a final evidence report containing:

- root causes,
- architectural changes,
- deleted duplicate/legacy paths,
- exact debt-baseline reductions/removals,
- rule coverage matrix,
- coverage numbers for changed code,
- cross-surface contract proof,
- exact commands/results,
- commits/push/CI/deploy evidence,
- zero-registration demonstration,
- remaining external-only blockers.

The final repository state must embody this principle:

> Discover/model once, enforce once, execute once, prove once, derive everywhere.
