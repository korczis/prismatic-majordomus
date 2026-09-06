# Measurement. `bench-run` times every externally callable operation; `bench` runs the
# criterion microbenchmarks. Numbers in this repository are measured, never asserted.

# Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations). `just bench-run objects.search --transport mcp --profile full` narrows it.
[group('bench')]
bench-run *args: build
    "{{rust_bin}}" bench {{args}}

# Benchmark coverage: covered / required, the denominator generated from the registry; exit 10 when anything is missing or waived.
[group('bench')]
bench-coverage *args: build
    "{{rust_bin}}" bench coverage --check {{args}}

# Compare a run with this platform's accepted baseline under .ai/repo/benchmarks/rust/ (policy.yaml); exit 10 on a regression.
[group('bench')]
bench-check *args: build
    "{{rust_bin}}" bench --profile ci --check --no-write {{args}}

# Record this platform's baseline from a full run (a reviewable, tracked file); refuses a dirty tree.
[group('bench')]
bench-baseline *args: build
    "{{rust_bin}}" bench baseline update {{args}}
