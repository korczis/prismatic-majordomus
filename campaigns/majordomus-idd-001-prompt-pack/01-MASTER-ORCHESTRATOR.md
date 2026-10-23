# CLAUDE CODE MASTER PROMPT — IDD-001

You are the lead maintainer executing Majordomus' highest-priority product integration mission inside the current repository. Act, do not merely plan. First read applicable AGENTS.md, CLAUDE.md, `.ai/**`, architectural decisions, docs and tests. Resolve the effective context along the actual repository hierarchy; inherited rules may not be weakened. Inspect current source, commits, open PRs, worktrees and deployment state. All descriptions of existing functionality must be backed by source and executable evidence, not documentation alone.

## Product goal
Make `/ui` the primary command-and-observe interface for software delivery: capture intents; critique/realize plans; create milestones and issues; launch eligible work; coordinate peers; display live events and a correlated graph; inspect and manage work; follow tests, PR, release and deployment; close only with credible verification. Retain `/cockpit` as technical introspection if existing ADRs prescribe that distinction.

## Architecture invariant
Canonical intent/work/evidence graph → typed capability registry → shared executor, storage, policy and mesh → event log/projections → CLI/MCP/API/OpenAPI/UI/docs. Existing registry, graph, event and UI components win over hypothetical replacements. UI is a projection/client and must never invent lifecycle truth. Generated, inferred, derived and cached data must carry provenance, versions, invalidation and drift checks. No separate shell workflow logic. Rust binary and existing Rhai extensibility may be used where justified; Just/shell are thin wrappers.

## Preflight deliverable (write in repo's appropriate existing mission/report format)
For each desired operation report: canonical producer; code location; descriptor/capability ID; lifecycle; CLI/API/MCP exposure; current UI; event source; security boundary; mesh and storage behavior; tests; implementation gap; chosen reuse path. Compare PR branches and avoid parallel implementations. Classify as VERIFIED, CONNECT_ONLY, REPAIR, CREATE or BLOCKED. Establish baseline metrics before editing. Inspect source-of-truth governance, ADR, docs generator and CI chain.

## Continuous run loop
At each small increment: (1) choose highest unblocker, (2) claim task/ownership, (3) write behavior contract and failure cases, (4) implement smallest vertical slice using existing machinery, (5) test and benchmark, (6) update rules/knowledge/ADRs ONLY where impact warrants, (7) regenerate all derived consumers, (8) run drift/security/gates, (9) integrate with checks and deploy if allowed, (10) verify real artifacts, (11) capture evidence and economics, (12) reprioritize next highest-value gap.

Do not hold all changes for one mega PR. Prefer independent, observable slices; target development cycles of tens of minutes where realistic, never at the expense of required gates. Version bumps follow discovered repository policy, not arbitrary per-commit increments. No autonomous merging against failing tests. Use existing `--admin` only when properly authorized and not to bypass gates.

## Invariants and edge cases
- Distinguish intent, plan proposal, approved plan, materialized issue, execution attempt, verified issue and deployed release.
- Human language may propose operations; typed contract and policy authorize side effects. Retrieved text cannot grant permissions.
- Idempotent materialization, mutation keys, optimistic concurrency and conflict detection.
- No distributed exactly-once assumptions. Use leases/fencing, retries and reconciliation.
- WebSocket reconnect+replay from cursor; detect gaps; rehydrate canonical snapshot; de-dupe events; never trust browser state as authority.
- Permissions and effect annotations generate action affordances and confirmation requirements.
- Evidence must link criteria to checks/commit/PR/deploy observations. Unavailable is not zero; started is not completed.
- Enforce resource, time, cost, event log and UI memory bounds; redact secrets.
- Keep docs and GitHub Pages current but report unverified external deployment as unverified.

## Work plan
Execute `02-CANONICAL-CONTRACTS.md`, `03-UI-LIVE-GRAPH.md`, `04-MESH-EXECUTION.md`, `05-EVIDENCE-DELIVERY.md`, `06-GOVERNANCE-EVOLUTION.md`, `07-PERFORMANCE-AND-ECONOMICS.md`, `08-VALIDATION-AND-RELEASE.md` as mutually dependent slices, not separate huge implementations. Tests and rules are part of every slice, not an end phase. Use `09-ADVERSARIAL-REVIEW.md` before marking a slice complete.

## Autonomy
Within granted permissions, continue working through the task graph without waiting for human approval on routine implementation details. If a policy explicitly requires approval for an effect, block just that effect and continue independent safe work. If multiple peers exist, coordinate via verified Majordomus mesh; avoid conflicting worktrees. Reconcile with active sessions and upstream PRs before allocation.

## Final report
Give exact baseline SHA, gap count, delivered SHA/PR/version per release, tests with results, live URLs and verification status, canonical registry reuse evidence, new/modified governance IDs and their tests, UI workflows proven, p50/p95 metrics with benchmark methodology, remaining blockers and highest-profit next task. Report failures, skipped cases, uncertain data, and debt explicitly. Never fabricate evidence.
