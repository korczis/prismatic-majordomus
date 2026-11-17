# Milestone 02: Incremental derive and bounded work

Review live implementation of `scripts/derive`, Rust generation, docs/site pipeline, `just` modules, derived merge driver, current lock in #855 and tests before touching anything. Primary observed risk: commit f1a892eb reports 699s derive on Linux and >1h on loaded Mac; verify current baseline. DO NOT remove single-flight protection simply to get speed. Prefer stable canonical dependency metadata over filesystem heuristics.

## Issue 02.1: Trace the derive DAG

Inventory derived artifacts, authoritative inputs, generators, output schemas, transitive edges and consumers. Build dependency/impact graph by extending existing command/capability/dependency graph if suitable; reject hidden inputs. Prove that generating A cannot unexpectedly mutate output B. Keep checked-in generated artifacts consistent with repo policy.

## Issue 02.2: Deterministic content addressing

Use content digests over canonicalized input blobs, tool/generator versions, relevant flags and environment constraints. Cache keys include every meaningful dependency. Distinguish machine-specific outputs from portable outputs. A missing/corrupt cache triggers recomputation; never silently accept. Cache storage location, eviction and permissions follow current policy. A changed-only invocation must preserve byte-for-byte parity with a clean full regeneration.

## Issue 02.3: Precise invalidation and bounded parallelism

Changed rule invalidates only affected rules/projections; unrelated site assets remain untouched. Independent nodes parallelize within measured resource limits. Shared identical work single-flights by exact key, while conflicting writes serialize by output resource/lease. Lock timeout, process death, signals, stale lock and abandoned temp files handled safely. Atomic publish with no partial generation. No network dependency during local no-op derive unless existing contract explicitly requires it.

## Issue 02.4: Fast-check and full-check equivalence

Preserve full authoritative gate and add/select a reliable incremental path. Prove: incremental PASS implies equivalent relevant full validation for the same inputs. Run randomized dependency order, add/remove/rename input, generation failure, interruption, concurrent branches, cache poisoning, cross-platform line-ending tests. Detect output drift, orphans and phantom changes. Preserve release-friendly reproducibility.

## Issue 02.5: Performance contracts and docs

Collect median/p95 no-op and representative changed-input times (minimum repeatable sample and machine metadata), number of nodes rebuilt, saved work, critical path. Compare with baseline, prohibit unbounded regression with portable threshold logic (noise-aware, not brittle absolute target). Wire into existing CI/doctor and document exactly when slow full validation is required. No manual generated artifact inventories.

Acceptance: no-op is dramatically cheaper by measurement; a single canonical change does not trigger whole-world regeneration; crash and concurrent tests preserve canonical outputs; clean full derive and incremental derive produce identical artifacts.
