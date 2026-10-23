# Majordomus P0 Performance Rescue — Prompt Pack

**Purpose:** deliver one measurable performance release train for the real human-facing `majordomus` CLI, `/ui`, and `/cockpit`, not add another subsystem.

## Fast start in Claude Code

1. Open Claude Code in the actual `prismatic-majordomus` checkout, check the active branch and context.
2. Paste the complete `MASTER-PROMPT.md` into the session (or point Claude Code at that local file and ask it to execute it).
3. Give Claude access to the pack's `prompts/` and say: **Execute phases `00` through `18` against current HEAD. Do not stop at planning; show test and performance evidence.**
4. A specific phase may be handed to another mesh worker only after scope/claim coordination. One integration owner must keep source and generated projections consistent.
5. Keep the work within the existing canonical issue/intent/milestone workflow. Do not let an autonomous worker commit to someone else's worktree or bypass CI.

## Critical discovery

The reviewed `readings.rs` allows a cold first reading to wait 60 seconds and uses 4 shared workers. UI warming processes every screen and panel in the background. CLI `env enter` can also ensure a runtime and refresh the workflow bridge. These are **test leads**, not an established root cause. The existing warm UI benchmark p95<=100ms does not prove cold/browser responsiveness. See `SOURCE-MAP.md`.

## Phases

00. `prompts/00-governance-and-freeze.md`: Governance, source truth, and scope freeze
01. `prompts/01-reproduce-and-baseline.md`: Reproduce end-to-end latency and build the baseline
02. `prompts/02-cli-and-direnv-entry.md`: Eliminate CLI and direnv entry overhead
03. `prompts/03-shared-runtime-startup.md`: Bound shared runtime startup, readiness and lease convergence
04. `prompts/04-ui-cold-readings-first-paint.md`: Fix cold /ui first reading without lying about freshness
05. `prompts/05-cockpit-route-and-navigation.md`: Remove expensive /cockpit SSR, nav and overview work
06. `prompts/06-expensive-capabilities-and-git.md`: Stop repeated multi-second plan and Git derivations
07. `prompts/07-http-concurrency-backpressure.md`: Fix resource contention without rewriting the HTTP stack
08. `prompts/08-realtime-revalidate-and-offline.md`: Make live refresh reliable without repeated full reads
09. `prompts/09-browser-waterfall-and-responsive.md`: Measure and repair real browser experience
10. `prompts/10-cache-freshness-and-equivalence.md`: Make caching correct, typed, bounded and observable
11. `prompts/11-request-tracing-server-timing.md`: Implement I3226 request-scoped causal timings
12. `prompts/12-bench-policy-and-hard-gates.md`: Enforce user-visible SLOs through existing benchmark policy
13. `prompts/13-mesh-and-external-dependency-isolation.md`: Keep mesh, forge and model/provider outages off read hot paths
14. `prompts/14-integration-release-train.md`: Integrate measured repairs as one release train
15. `prompts/15-production-and-browser-delivery.md`: Validate exact deployed bits on hosted dynamic and static surfaces
16. `prompts/16-chaos-scalability-and-soak.md`: Prove cold, churn, load and failure resilience
17. `prompts/17-derived-docs-and-observability.md`: Project verified performance truth everywhere it belongs
18. `prompts/18-final-adversarial-acceptance.md`: Close only with demonstrated user-visible speed

## Contracts and evidence

- `CONTRACTS.md`: correctness, latency measurement, caching and security.
- `contracts/acceptance-matrix.md`: strict PASS/FAIL/BLOCKED/NOT_RUN table.
- `contracts/benchmark-methodology.md`: sample design and measurement traps.
- `contracts/issue-and-release-train.md`: existing issue integration and release semantics.
- `contracts/failure-injection.md`: failure and scaling drills.
- `evidence/evidence-template.json`: typed example evidence fields, not fabricated results.
- `evidence/final-report-template.md`: final report layout, fill only measured values.
- `SOURCE-MAP.md`: source map verified 2026-10-23.

## Operational truths

A GitHub-connected source review is not a benchmark of your machine or a deployed website. No source files have been changed by this pack. Tasks demand actual implementation and proof from the executing agent. Numeric budgets beyond the repo's existing warm p95 100ms are provisional until real runs justify them. No fake RUN/PASS flags.
