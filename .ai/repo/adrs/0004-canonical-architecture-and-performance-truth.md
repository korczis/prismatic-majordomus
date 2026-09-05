# 4. One canonical declaration, composed modules, derived projections, and performance as evidence

Status: accepted, 2026-09-05. Extends ADR 2 and ADR 3.

## Context

ADR 2 made every interface of the Rust executable a projection of one capability
registry. Two things were still written by hand or not written at all: the registry was
one flat list of every executable, and nothing measured. The operator's mission was
explicit: a contributor adding one capability must edit one canonical declaration and
nothing else, no interface may carry a second copy of a fact, every externally callable
operation must be benchmarked with a denominator nobody types, a request must never
rebuild canonical state, caches must be proved equivalent, and every claim about speed
must come with numbers. The MCP surface was measured before any of this: `capabilities
list` took 166 ms per call over MCP and 57 ms over HTTP on this repository because the
listing shipped the same object-view schema for every one of two hundred declarative
resources, `resources/list` took 15 ms because it walked the registry per request, and a
cold `majordomus mcp` took a quarter of a second to answer its first `tools/list`.

## Decision

- **One declaration.** `capability!` is the whole declaration of an executable: id,
  optional kind (`query`, or `command` for one that changes this process's memory),
  title, description, typed input and output, stability, exposure, tags, optional cache
  policy, optional benchmark policy, handler. The input type implements `BenchmarkCases`,
  so a capability without a benchmark input does not compile. Schemas are inferred from
  the types; MCP names, OpenAPI operations, Swagger UI, the command line, the benchmark
  targets, the cache behaviour and the generated documents are derived. Nothing is written
  a second time.
- **Modules compose capabilities; the root composes modules.** `module!` builds a
  `ModuleDescriptor` (id, title, description, stability, executables) and stamps its id on
  each capability; `compose_modules![repository, objects, capabilities, peers, perf]` in
  `builtin/mod.rs` is the only root composition, and a capability added inside a module
  needs no root edit. The registry refuses a capability composed in a module other than
  its id's namespace, a module composed twice, an invalid module id, a cache policy that
  keeps nothing, a cached command, and a benchmark policy that contradicts the kind.
  Declarative objects join the same registry with their kind as module; nothing has a
  second registry.
- **Macros generate values, not registries.** `capability!`, `module!` and
  `compose_modules!` are `macro_rules!` that build plain values handed explicitly to the
  registry builder in `app.rs`. No procedural macro, no global mutable registry, no
  link-time inventory, no build script scanning sources.
- **One executor.** Every call through every transport, the command line and the
  benchmark runners goes through `CapabilityExecutor::execute` (`Context::execute`):
  registry lookup, counters, the cache the descriptor asks for, the handler. Transports
  convert protocol and own nothing else. The raw `registry.dispatch` exists for the
  executor alone.
- **Cache is policy on the descriptor.** `CachePolicy::Process { max_entries, ttl }`
  keeps results in process memory, keyed by the canonical id, the input in canonical form
  (object keys sorted at every level) and the registry fingerprint (a sha-256 of the index
  fingerprint, itself over every object's path and content, and of every descriptor).
  Errors are never cached; commands are never cached; nothing is persisted. Two
  capabilities declare it because measurement said so: `objects.search` and
  `capabilities.list`.
- **Performance is counted, then measured.** `perf::COUNTERS` counts the work that must
  happen once (repository scans, index and registry builds, schema generations, MCP
  projection builds, OpenAPI builds, HTTP router builds) and the work that happens per
  call (executions, handler invocations, cache hits, misses, evictions), with phase
  timings on a monotonic clock; `perf.counters` answers them over every transport. The
  structural tests send hundreds of requests through the real transports and require the
  startup counters unchanged. The MCP tool and resource listings are prepared once;
  `capabilities.list` answers summaries and the full descriptor is one `describe` away;
  the object-view schema is derived once per process.
- **Every operation is a benchmark target.** `BenchmarkProjection` derives, from the
  registry against a repository, one target per required executable per transport it is
  exposed on per case of its input type, plus the transports' own operations declared once
  as `SystemTarget`s. Coverage is `covered / required` with a generated denominator;
  waivers are typed, reported and never counted; `capabilities validate` fails on a missing
  requirement. Runners time directly through the executor (cold and warm for a cached
  capability), over a real loopback socket with the input bound as the route binds it, and
  through a real `majordomus mcp --standalone` child with a fresh process per sample for
  the process-cold target. Results are a versioned document with commit, dirty state,
  build profile, platform and registry fingerprint; local runs go under
  `.ai/local/benchmarks/`, the accepted baseline is one tracked file per platform under
  `.ai/repo/benchmarks/rust/`, promoted only by `bench baseline update`, and the
  regression policy is data beside it. A stale baseline entry is reported by name, never
  attached to a renamed target.
- **Generated artifacts are derived and reconciled.** `docs/generated/openapi.json`, the
  reference index and one page per executable module, `benchmarks.md` and
  `registry.json` are written by `majordomus generate` and compared byte for byte by
  `generate --check`; every file says it is generated and names its source; no timestamp.
- **Property-based guarantees.** `proptest` strategies are derived per executable from
  its benchmark cases and the fields its input type has; one strategy feeds the cache
  equivalence, the key normalisation, the transport equivalence, the projection
  determinism and the duplicate-detection properties.

## Against the "Intentionally Absent" list, point by point

| the list refuses | this decision |
|---|---|
| a daemon, server, background monitor | nothing new: the counters and the cache live in the process ADR 3 already bounded to its clients |
| a database, queue, vector store | the cache is bounded process memory; benchmark evidence is files a person reads and git reviews |
| a registry or catalogue of named workers | the registry indexes the tool's own capabilities and the repository's objects, as ADR 2 said; modules are Rust modules |
| a finding without a reproduce command | every refusal of the registry names the id, the provenance and the reason; `bench coverage`, `bench --check` and `generate --check` name every missing, stale or regressed item |
| a self-report trusted without an independent check | latency is measured through the real transports by a child process and a real socket; the hot path is proved by counters read after real requests, not by an assertion in the code that claims to be fast |
| a number written where a command could compute it | the coverage denominator, the tallies, the fingerprints and every latency are computed; the generated files carry no count a person typed |

## Invariants

1. A capability is one `capability!` block; changing its id, title, description, input,
   output, exposure, cache or benchmark policy changes every projection and no other file
   (`tests/projections.rs`, `tests/bench.rs`).
2. The root composes modules and only modules (`tests/registry.rs`).
3. Every executable is a benchmark target directly and on every exposed transport, with
   the cases its input type provides; coverage is complete or `validate` fails
   (`tests/bench.rs`, `capabilities validate`).
4. After startup, no request scans the repository, builds the index or the registry,
   derives a schema or builds a projection (`tests/hot_path.rs`).
5. A cached capability answers the same value uncached, cold and warm; a hit runs no
   handler; errors and commands are never cached; the bound holds; key order is
   irrelevant; the fingerprint separates repository states (`tests/executor.rs`,
   `tests/properties.rs`).
6. Every generated artifact is deterministic and reconciled (`generate --check`).
7. A baseline is compared only on its own platform, under a policy that is data, and
   never silently attached to a renamed target.

## Alternatives rejected

- **Procedural attribute macros** (`#[majordomus::capability(...)]`). They would buy a
  shorter spelling at the price of a second crate, a compile-test harness and generated
  code nobody reads; the `macro_rules!` declaration already holds every fact once and
  fails to compile without a benchmark case. Reversible: the descriptor the macro builds
  is the contract, not the macro's syntax.
- **Link-time registration** (`inventory`, `linkme`) and **build scripts scanning
  `src/`**: composition would stop being readable in one file; refused by the operator's
  own brief as well.
- **A boolean `read_only`** instead of the `command` kind: three projections would each
  have consulted it; the kind carries the semantics once (ADR 3).
- **Caching every capability by default.** Most handlers answer in microseconds from an
  immutable index; a cache there is a second copy of nothing. The two cached ones were
  measured first.
- **A persistent result cache.** Invalidation across processes would need what the
  fingerprint gives for free within one; restart-based rediscovery remains the contract.
- **A hardcoded expected count of benchmark targets** in a test: the denominator is the
  registry's; a count typed by a person is the drift the mission forbids.
- **One benchmark inventory file** beside the code: it would be the second registry the
  whole design refuses; the projection is computed from the descriptors.
- **Wall-clock thresholds hardcoded in CI**: environment noise would make them either
  vacuous or flaky; the policy is data, the baseline is per platform, and the structural
  counters are the hard gate.

## Consequences

Adding a capability to an existing module is one `capability!` block, its typed input
and output, and the input's `BenchmarkCases`; `majordomus generate` refreshes the
committed projections and `capabilities validate` proves the rest. Adding a module is one
Rust module with `module()` and one name in `compose_modules!`. The registry fingerprint,
the counters, the benchmark result and coverage schemas, the target keys and the
generated file set become compatibility surfaces. `capabilities.list` no longer carries
schemas; a client that needs one calls `capabilities.describe`. A shared server keeps the
index it built at start; the counters make that visible rather than hidden.

## Testing strategy

Black-box first: `tests/hot_path.rs` (counters across hundreds of real requests),
`tests/bench.rs` (derivation, exposure and policy propagation, the three runners, the
check, the command line), `tests/executor.rs` (cache semantics through the executor),
`tests/properties.rs` (property-based, registry-driven), `tests/projections.rs` (every
projection and every generated artifact), `tests/registry.rs` (module invariants),
`test/cases/91_canonical_architecture.sh` (the same through the built binary in a
repository `init` wrote). Benchmarks: `benches/projections.rs`, `benches/shared.rs`,
`benches/scaling.rs` over the synthetic repository, and `majordomus bench` itself. The
gates of ADR 2 gain `bench coverage --check` in `scripts/rust-check` and CI.
