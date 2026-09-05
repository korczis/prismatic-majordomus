# After startup, no MCP or HTTP request of the Rust executable scans the repository, builds the index or the registry, derives a schema or builds a projection, and perf.counters proves it

## What it means

Everything canonical is built once, when the process starts: the repository is discovered, the index built, the registry composed and validated, the MCP listings and the OpenAPI document prepared on first use and kept. A request looks things up and runs a handler. The process counts its own work and answers the counts over every transport as `perf.counters`; a test sends hundreds of requests and requires the startup counters unchanged, so a refactor that quietly rebuilds the registry per request fails before anyone measures latency.

## How it works

`apps/majordomus-cli/src/perf.rs` holds the process-wide counters (repository scans, index builds, registry builds, schema generations, MCP projection builds, OpenAPI builds, HTTP router builds; executions, handler invocations, cache hits, misses, evictions) and the phase timings on a monotonic clock, each incremented at one named place; `capability/executor.rs` is the one execution path every transport uses; `mcp/surface.rs` prepares the tool and resource listings once per shared listing; `http/router.rs` renders the OpenAPI document once; the `perf` module exposes the counters as a capability.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus serve &                      # then, after many requests:
curl -s http://127.0.0.1:8741/api/v1/perf | jq '{repository_scans, index_builds, registry_builds, schema_generations, mcp_projection_builds, openapi_builds}'
# the same numbers after the hundredth request as after the first
```

## What it does not cover

A shared server keeps the index it built at start; a change to the repository is seen by the next server, not by this one (restart-based rediscovery is the contract, ADR 2 and ADR 3). Wall-clock latency is the benchmark's business; this claim is about the work a request does.

## Why it exists

The slow path the mission started from was work done per request that belonged at startup, and a feeling is not a regression test. `project.rust-hot-path` is the rule, ADR 0004 the decision. `apps/majordomus-cli/tests/hot_path.rs` sends two hundred requests through the real stdio and HTTP transports; `test/cases/91_canonical_architecture.sh` sends two hundred frames to the built binary in a repository `init` wrote and compares the counters before and after.
