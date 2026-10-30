# Mission 08 — measure before optimizing; fast feedback at controlled cost

Audit `docs/PERFORMANCE.md`, MJ_TIMING, baseline, actual commands and benchmarks. Historical sources discuss batched awk hashing/flattening and shell forks; verify current hotspots. Do not assume user goals are already met.

1. Run controlled cold/warm baselines for CLI, `doctor`, `check`, `finish`, `init`, `generate`, API response, UI TTFB/first useful render, SSE/WebSocket updates, memory/CPU, mesh journal fold/reload and full CI feedback. Capture host hardware, OS, toolchain, sample size, cache mode, SHA, data size.
2. Use p50/p95/p99 only with adequate sample size; treat small samples as descriptive not SLA proof. Budget thresholds must be justified by product use and baseline, not blindly copied from prior speculative targets.
3. Optimize highest measured waste: process-per-item, repeated scans, duplicated state, nondeterministic derives, serializable parsing, avoidable network/compilation, hot endpoint fanout, heavy frontend JS.
4. Compare HTTP stacks (`tiny_http` vs axum per I2488 #1075) with same route mix, several hundred streams, memory cap, stalls and graceful shutdown; migrate only with demonstrable improvement and migration budget. Don't rewrite for prestige.
5. Use project-approved two-level caching; shared mesh build cache remains optional and must fail-open to local builds. PR #1118 describes typed offer selection but explicitly no full integration. Track actual MinIO/sccache health and provenance; no embedded credentials.
6. Record economics: compute cost per verified outcome only where token/compute data and attribution actually exist. Missing provider telemetry => UNKNOWN, never estimated as billed. Costs estimated vs reconciled separately.
7. Prevent regressions via runner-aware benchmark policy, stable fixtures, cache invalidation and warmed/cold semantics. Show `before vs after` using identical task/context/hardware where feasible.

Evidence artifacts: machine-readable baseline, flamegraph/tracing where supported, bottleneck ranking, diffs, limits and proposed budgets. Acceptance: at least critical speed regressions fixed or explicitly blocked with reproducible evidence, fast path never requires external mesh/provider, benchmark and CI tests enforce meaningful regressions.
