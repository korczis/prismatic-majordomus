# Phase 12: Enforce user-visible SLOs through existing benchmark policy

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 12 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Read existing `.ai/repo/benchmarks/rust/policy.yaml` (warm UI p95 100 ms / min 20 samples), `src/bench/{baseline,coverage,projection,results,runner,stats,system}.rs`, `src/commands/bench.rs`, `scripts/rust-check`, project policies `project.performance-evidence`, `project.rust-hot-path`, `project.benchmarkable-commands`, `project.blocking-checks-cheap`, docs performance. Add process-cold CLI, `env enter` and native launch, cold first reading, warm `ui` and `cockpit` SSR, latency after Git change, browser first-use, reconnect, and degraded case *without hand-authored route inventories*. Integrate proper target coverage with the existing registry/projection.

Distinguish aspirational SLO from hard checked budget: do not write a threshold without a measured baseline/run identity. Keep p95 with >=20 samples; p99 require enough samples (policy currently >=200); regression thresholds and absolute limits are separate. Stratify by host/build profile, avoid noisy shared CI wall-time gates; use deterministic local fixture/in-memory budgets for blocking checks and report external shared-runner values as advisory when warranted. Protect correctness gates and enforce structural `hot_path.rs` always. A benchmark skipping broken routes or passing with no browser is a critical defect.

## Proof / exit criteria

Deliver a validated policy diff, auto-generated benchmark coverage, before/after reports, explicit sample count and CI failure examples, one intentionally induced regression that makes gate fail, a baseline-update procedure requiring review, and < reasonable CI budget. No manual fake baseline JSON or unreviewed auto-accept of slower output.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
