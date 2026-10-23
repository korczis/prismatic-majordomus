# Benchmark Methodology and Truthfulness Rules

Record repository SHA, executable binary digest, build profile, OS, CPU, available RAM, load, seed fixture, cache state, browser version, network origin and timestamp. A benchmark value without these has no authority.

## Separate dimensions

- **Process cold:** spawn a new executable for each sample; avoid reusing the same running CLI instance. Note FS/OS cache state separately (cold process != uncached filesystem).
- **Server cold:** new runtime + empty `readings` and memo. Different from an isolated new HTTP request.
- **Server warm:** repeat same route on same source generation after deliberate warmup; record both socket and handler timing.
- **Source changed:** a Git branch/index/plan/lease change invalidates selected state; measure post-change requests.
- **Browser cold:** fresh context/page, browser cache and service worker state noted; warm is same browser context.
- **Degraded:** source stalls/remote offline/peer churn; distinguish deliberate timeout states from fast success.
- **Hosted:** per origin, direct network baseline, repeat samples; separate network RTT/TTFB from render and asset costs.

## Statistics and comparison

Use monotonic wall clocks; warmup separately; report min, median/p50, p90, p95, p99 (if enough N), max and sample count, plus distribution and confidence/variance as practical. Existing benchmark policy only judges p95 at >=50 for regression and UI absolute budgets at >=20; p99 only >=200 samples for regression. Do not compute a meaningful p99 from 5 runs. Compare like with like: same host, build profile, SHA-run identity, fixture and cache definition. For uncontrolled shared GitHub CI wall time, report trend but don't hard-fail correctness as proposed ADR 0196 cautions.

## Stage attribution

Instrument request accept/queue, route, canonical live view, source queue wait, source execution/cache, SSR/nav, serialization, response bytes/write and browser fetch/paint/long tasks. Global atomic counters are not valid request-scoped timers under concurrency. Attach Server-Timing where protocol permits and correlate structured event for stages after headers. Measure instrumentation overhead.

## Repro guidance

The repo already supports `MJ_TIMING=1`, `majordomus bench --help` / `--check`, `scripts/ui audit --origin` and canonical browser drivers. Query the executable's help before assuming argument spelling. Don't introduce shell scripts to duplicate benchmark runner semantics. Do not rewrite platform baselines by hand; use `majordomus bench --write-baseline` with a reviewable diff and provenance.
