# Every externally callable operation of the Rust executable is a benchmark target with a denominator generated from the registry, and a missing case fails the structural check

## What it means

Nobody keeps a list of what to benchmark. Each executable capability with a required policy is a target directly and on every transport its exposure declares, with the cases its input type provides; the transports' own operations (a cold `majordomus mcp` process, `initialize`, `ping`, `tools/list`, `resources/list`, `resources/read`, `GET /`, `/openapi.json`, `/docs`) are targets declared once. `majordomus bench coverage` reports covered over required, where required is computed from the registry; a capability whose input type produces no case for the repository at hand is missing, and `capabilities validate`, `bench coverage --check` and CI fail on it. A waiver is a typed reason on the descriptor, reported and never counted as covered.

## How it works

`apps/majordomus-cli/src/bench/projection.rs` derives the targets from the registry and the case providers; `bench/coverage.rs` computes the requirements and the tallies; `bench/runner.rs` times a target through the executor, over a real loopback socket with the input bound as the route binds it, and through a real `majordomus mcp --standalone` child; `bench/results.rs` and `bench/baseline.rs` hold the versioned result document, the per-platform baseline and the regression policy read from `.ai/repo/benchmarks/rust/policy.yaml`; `commands/bench.rs` is the command line.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus bench coverage                    # per transport and in total: covered / required, missing 0, waived 0
apps/majordomus-cli/target/debug/majordomus bench --profile quick             # every target, slowest p95 first; written under .ai/local/benchmarks/
apps/majordomus-cli/target/debug/majordomus bench objects.search --transport mcp --profile full --format json
apps/majordomus-cli/target/debug/majordomus bench --profile ci --check        # against .ai/repo/benchmarks/rust/baseline.<platform>.json, under the policy
```

## What it does not cover

Wall-clock numbers depend on the machine: a baseline is compared only on its own platform, and CI runners without a committed baseline compare nothing. The process-cold target is noisy by nature; the policy's absolute floor and per-metric thresholds are data a person tunes with evidence. The MCP transport measures a warm process for a cached capability (cold is not observable from outside); the direct transport reports cold and warm.

## Why it exists

"Representative endpoints only" is how the slow one escapes; the operator asked for every operation, with the denominator generated. `project.rust-benchmark-coverage` is the rule, ADR 0004 the decision. `apps/majordomus-cli/tests/bench.rs` proves the derivation, that an exposure added or removed adds or removes the requirement and the target with no other edit, that the three runners time real transports, and that a check reports every line and never attaches a stale baseline to a renamed target; `test/cases/91_canonical_architecture.sh` recomputes the denominator from the live registry and compares it with the coverage document.
