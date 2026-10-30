# Majordomus Incremental Derivation — Executable Prompt Pack

Goal: replace redundant Stage A → B → C → D full regeneration with a typed, content-addressed, safe incremental derivation DAG, while retaining correctness, `.ai` enforcement and live documentation.

## Execution

1. Begin with `00_MASTER_ORCHESTRATOR.md`.
2. Run the remaining numbered pages in order, each as a separate Claude Code task/session or sequential continuation with evidence checkpoint.
3. Treat every page's exit gate as mandatory; never mark a phase complete from code presence alone.
4. Final status is YES only when changed-code output equivalence, tests, CI, documentation, Pages verification where authorized, and measured speedup are proven.

The pack is source-informed by the current documented `scripts/derive`, `scripts/derive-check`, and Rust `generation.rs` architecture inspected on 2026-10-12. Reinspect HEAD, as source is changing rapidly.

## Index
- [00 — Master execution contract and continuation](00_00_MASTER_ORCHESTRATOR.md)
- [01 — Repository audit, DAG inventory and baseline](01_01_AUDIT_BASELINE.md)
- [02 — Typed canonical DerivationGraph and discoverability](02_02_TYPED_GRAPH.md)
- [03 — Merkle fingerprints, tracked working trees and validity](03_03_HASHES_FINGERPRINT.md)
- [04 — Incremental scheduler, skip logic and full fallback](04_04_INCREMENTAL_EXECUTOR.md)
- [05 — Persistent CAS, output restoration, integrity and GC](05_05_CAS_STORE.md)
- [06 — Operator commands: plan, explain, status, metrics, doctor](06_06_CLI_EXPLAIN.md)
- [07 — Granular site and docs invalidation](07_07_SITE_DOCS_GRANULARITY.md)
- [08 — Cross-worktree concurrency, leases and execution safety](08_08_PARALLEL_WORKTREES.md)
- [09 — GitHub Actions reuse, CI planning and artifact proof](09_09_CI_GITHUB_CACHE.md)
- [10 — Differential oracle, adversarial tests, .ai doctrines](10_10_CORRECTNESS_ENFORCEMENT.md)
- [11 — Derived UI, generated documentation and GitHub Pages](11_11_UI_DOCS_PAGES.md)
- [12 — Default enablement, release, live verification and final convergence](12_12_ROLLOUT_PROVE.md)
