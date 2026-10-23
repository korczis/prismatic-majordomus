# Phase 01: Reproduce end-to-end latency and build the baseline

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 01 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Use real executable paths; separate `bin/majordomus` Bash lifecycle tool, `bin/majordomus-env`, direct Rust binary, `.envrc`/direnv, shared-server `serve`, `/ui`, `/cockpit`, and relevant API/MCP operations. Read `docs/PERFORMANCE.md`, `apps/majordomus-cli/src/bench/{runner,system}.rs`, CLI bench args, `.ai/repo/benchmarks/rust/policy.yaml`, `scripts/ui`, `scripts/lib/browser.mjs`. Do not take a profile flag on faith: obtain `--help` first.

Capture cold process and hot process separately, warm and cold server UI, first reading vs already-loaded reading, post-git-change, offline GitHub, unreachable mesh, single click/navigation and simultaneous browser asset requests. Log machine OS/CPU/memory, host load, release-vs-debug, SHA, remote origin, fixture size and browser version; record variance, counts, p50/p95/p99 only with adequate N, max, TTFB, document load, FCP/LCP, JS long tasks, bytes sent and source-specific timings. Use wall-clock measurements that include subprocess and network stack, not just `Context::execute`.

Inspect the browser's network waterfall, console, websocket and refresh behavior. Label the cached results that may be old. Never compare debug cold against release warm as a speedup. Run clean baseline before code changes. Exclude an unavailable deployed endpoint from pass/fail and report it BLOCKED, not 0 ms.

## Proof / exit criteria

Commit only allowed baseline/policy files through the existing benchmark workflow; retain machine-local traces and browser profiles under ignored run storage. Produce machine-readable `before.json` and human `BASELINE-REPORT.md` with reproduction commands and ordered top ten wall-time contributors. No fixes until at least one slow path is reproduced or a clear measurement blocker is documented.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
