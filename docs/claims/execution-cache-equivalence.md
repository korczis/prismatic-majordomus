# A cached capability of the Rust executable answers the same value uncached, cold and warm, a hit runs no handler, errors and commands are never cached, and the key carries the registry fingerprint

## What it means

A cache is declared on the descriptor (`CachePolicy::Process { max_entries, ttl }`) and lives in the one executor every transport uses, so MCP, HTTP and the command line share it and none has a cache of its own. For every cached capability and every case its input type provides, the value answered with the cache off, with an empty cache and with a warm cache is the same; a warm answer runs no handler; an error is computed every time; a command is never cached and the registry refuses a descriptor that asks; the cache is bounded per capability and evicts the oldest; the same input with its keys in another order is the same key; and the key carries the registry fingerprint, a hash of every object's content and every descriptor, so two repository states never share an entry.

## How it works

`apps/majordomus-cli/src/capability/executor.rs`: `CapabilityExecutor::execute` looks the capability up, normalises the input (`canonical_json`), consults the cache under the descriptor's policy, runs the handler through `registry.dispatch`, and stores the value; `index.rs` and `registry.rs` compute the fingerprints with sha-256; `perf.rs` counts hits, misses and evictions.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus capabilities describe objects.search --format json | jq .cache
apps/majordomus-cli/target/debug/majordomus bench objects.search --transport direct --profile quick   # cold and warm rows, the direct one with handler invocations
```

## What it does not cover

Nothing is persisted: the cache is one process's memory and ends with it. Only what a measurement justified is cached (`objects.search`, `capabilities.list`); the rest answers from the immutable index in microseconds and a cache there would be a second copy of nothing. A TTL is supported by the policy and used by no builtin today.

## Why it exists

A cache that is not proved equivalent is a lie waiting for an input. `project.rust-hot-path` is the rule, ADR 0004 the decision. `apps/majordomus-cli/tests/executor.rs` iterates every cached capability of the registry with every case; `tests/properties.rs` does the same over generated inputs with proptest, including shuffled key order; `test/cases/91_canonical_architecture.sh` sees the hits in the counters after repeated searches through the built binary.
