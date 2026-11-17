# PROMPT 07 — Delivery evidence, UI truth, adversarial evaluation

Goal: expose useful evidence and measure if Majordomus genuinely improves outcomes. Do not build a new dashboard simply to display attractive numbers.

1. Use existing canonical intent -> issue -> execution -> PR -> merge -> release -> deploy graph. Determine actual available joins and missing traceability. Add narrow typed missing fields only when source exists.
2. Record freshness-bearing evidence: observed_at, source URI, SHA/revision, status PASS/FAIL/PENDING/STALE/UNKNOWN, source confidence, validation method. Never equate agent assertion with independent proof.
3. Expose queue, CI, release truth, stalled intents and broken evidence links through existing CLI/API/MCP/UI projections; use existing live streaming rather than polling everywhere. Provide drilldowns and actionable error messages.
4. Before UI work, run a semantic smoke test on desktop and mobile if tooling available. Fix legibility, deep links, version identity and slow queries without frontend rewrite. Reuse current UI architecture.
5. Design minimal rigorous comparison: equivalent sample tasks with same model/environment, Majordomus enabled vs baseline without its supervisory controls. Capture success under independent tests, delivery verification, wall time, interventions, token/cost and regressions. Use randomized order or matched pairs where feasible; no claims of causal improvement from anecdote.
6. If campaign time does not permit full A/B, implement instrumentation and one reproducible test case, state that effectiveness remains unproven.

## Acceptance
A user can find an actual failed or stale claim, navigate to evidence and fix path; no shadow UI state; a truthful benchmark protocol and any observations are attributable to SHA and environment.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
