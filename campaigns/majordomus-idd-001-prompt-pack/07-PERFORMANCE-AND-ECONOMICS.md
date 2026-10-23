# MISSION 07 — MAX PROFIT / FAST TURNAROUND

Optimize verified outcomes, not raw commits, LOC, agent count or nominal token count. Baseline actual p50/p95 for UI SSR, event propagation/reconnect, graph updates, CI and PR cycle, intent-to-verified-close, CI cache hit and build/test durations. Determine current available telemetry; never invent spend or savings. Attribute cost to intent, milestone, issue, execution and peer only when correlation exists.

Prioritize by `(expected outcome impact × reuse breadth × confidence) / (implementation cost + risk + maintenance cost)`, while honoring correctness, security, and unbounded-regret constraints. Publish a ranked gap table and select highest-leverage unblocker each iteration. Explicitly challenge scope creep and unmeasured visual polish.

Efficiency opportunities: build DAG/Merkle fingerprints and content-addressed caches where justified; shared query projections; incremental graph/event materialization; bounded log pagination; lazy loading; minimal WebSocket payloads; avoiding provider calls during SSR; early focused validation and cached CI; no per-request subprocess launch; Rust-native logic with existing Rhai extension boundaries; no repeated shell wrappers. Always compare introduced complexity to measured benefit.

Performance initial *targets* to test, never baseline facts: local warm SSR p95 <200 ms if realistic, event-to-visible progress p95 <250 ms, moderate graph interaction near 60fps, bounded memory at 1k/10k graph nodes, reconnection without dropped committed state; adapt targets to existing benchmarks and operational conditions. Tests must capture regression thresholds and reproducible hardware/fixture settings.

Profit report every slice: before/after median cycle time, manual steps removed, capability reuse count, duplicate files/registries eliminated, test flake count, CI minutes, event lag, error rate, attributed spend if observed, expected versus actual benefit and next bottleneck. Negative results must stay visible.
