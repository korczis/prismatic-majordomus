<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `perf`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `perf` — Performance

This process's work counters and phase timings: what happened once at startup and what happens per call, for the structural tests and the benchmark evidence.

Stability: behaviorally_verified. Capabilities: 1.

## `perf.counters` — Performance counters

The counters of this process: repository scans, index and registry builds, schema generations, projection builds, executions, handler invocations, cache hits, misses and evictions, and the phase timings, as they stand now.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_perf` |
| HTTP | `GET /api/v1/perf` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::perf |
| tags | performance, introspection |

Input: none.

Output: `CounterSnapshot`.

