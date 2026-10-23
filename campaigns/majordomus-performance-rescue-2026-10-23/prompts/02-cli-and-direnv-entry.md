# Phase 02: Eliminate CLI and direnv entry overhead

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 02 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Read `.envrc`, `bin/majordomus`, `bin/majordomus-env`, `lib/rust_bin.sh`, `apps/majordomus-cli/src/main.rs`, `src/commands/env.rs`, `src/environment/**` (discover actual paths), `src/command_graph/**` and `src/commands/serve.rs`. Profile separately: shell interpreter startup and library source time; name-dispatch; `--help` and `version` fast paths; `Repository::discover`; policy read/parse; Git status; stale binary check; lease/socket probes; `ensure_runtime`; `refresh_bridge`->`load::root`/`load::full`; bridge materialization; banner renderer; stdout+stderr write.

Measure clean/dirty tree, 1/10/100 repeated `cd`, first after build, missing/stale Rust executable, no direnv, CI/no tty, `MAJORDOMUS_RUNTIME=off`, `MAJORDOMUS_BANNER=off`, peer listening/not listening, both debug/release profiles. Preserve: shell does not start compiler, `.envrc` is an adapter, no remote network on entry, no secret leaks, no stale executor silently claimed current, one entry invocation no duplicate read. If a bridge is dirty, prefer change-dependent incremental materialization outside user entry or proven cheap path. Do not create another Bash registry or turn a cheap command into `App::load`.

Implement only fixes supported by phase-timing attribution. If wrappers are retained, document why and cap cost; if a portion can be consolidated into the existing typed Rust entry, use current ADR/governance plan, not a parallel implementation. Ensure CLI output compatibility and exit status contract remain intact.

## Proof / exit criteria

Run `MJ_TIMING=1` and OS-level subprocess profiling plus bounded repetitions. Add regression tests covering real `bin/majordomus`/direnv entry cold, warm, stale, missing and daemon states. Report per-stage before/after and final p95. No domain logic in `.envrc`, no new shell scripts, no build/network on `cd`.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
