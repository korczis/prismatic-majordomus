# Operation SHELLFALL | Majordomus | Claude Code Opus/Fable

**Mission:** Replace repository-domain shell/Bash with canonical Rust capabilities + sandboxed Rhai policies/orchestration, derive `just` from the existing command graph, port shell tests to Rust/Rhai with assertion parity, and make all outputs typed/inferred/derived/generated/portable.

**Important historical baseline:** 2026-09-11 uploaded archive included 218 `.sh` files, root `justfile`, `.just` imports, optional generated `.ai/local/cache/command-graph/bridge.just`, Bash `bin/majordomus` and Rust CLI. These are **not current-head assertions**.

## Files / execution order

00. `00-audit-baseline.md`
01. `01-canonical-workflow-contract.md`
02. `02-rhai-runtime.md`
03. `03-derived-just.md`
04. `04-port-tests-to-rust-rhai.md`
05. `05-remove-shell-legacy.md`
06. `06-portability-cache-dx.md`
07. `07-enforcement-rollout.md`

## Invocation

Run phases sequentially inside Claude Code at repo root, using Opus/Fable high-effort mode. Provide the full contents of each phase as the instruction. Carry the implementation changes, tests, audit and evidence forward; do not reset context or reimplement a second subsystem. Verify each phase before running the next.

Example operator instruction: `Read 00-audit-baseline.md. Execute all instructions on current HEAD. Implement and verify; record exact results. Then report the next phase's prerequisites.`

## Non-negotiable architecture

```text
Repo + .ai canonical metadata + command graph + capabilities
                   |
              typed workflow
                   |
          canonical execution plane
          /        |          \
    Rust host   Rhai policy   events/evidence
                   |
              projections
        Just / CLI / UI / MCP / OpenAPI / docs / CI
```

Shell survives only as narrow unavoidable bootstrap/interop. `just` is an ergonomic consumer, not a workflow source of truth once migrated. Rhai never bypasses the existing executor, policies and evidence.

## Independent reference sources

- https://docs.rs/rhai/latest/rhai/struct.Engine.html
- https://just.systems/man/en/formatting-and-dumping-justfiles.html
- https://just.systems/man/en/modules.html
- https://just.systems/man/en/metadata.html

Consult the versions actually installed/pinned in the repository before using APIs/features.
