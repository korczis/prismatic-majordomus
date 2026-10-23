# Phase 18: Close only with demonstrated user-visible speed

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 18 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Run final acceptance from clean/rebuilt binary and server restart on the integrated commit. Repeat baseline matrix: CLI/direnv, Rust direct, `/ui`, `/cockpit`, primary routes/panels, incremental Git mutation, remote/mesh outage, browser desktop/mobile/no-JS, event-to-visible, heavy contention, security/CSP/authorization, cold and warm. Compare to baseline on same host/build/profile, with N for p50/p95/p99; report absolute and relative difference for each. Run cargo fmt/check/test according to project, shell tests, structural `hot_path.rs`, cache equivalence tests, `majordomus bench --check`, `scripts/ui` tests/audit, generated docs drift and current canonical quality gates. Update issue body/evidence not the other way around.

Check git diff and branch after all tests; ensure no new shell scripts, no extra provider calls in hot path, no manually duplicated projection inventories, no disabled CI checks, no secrets, no pending regressions hidden by cache, and no code paths accidentally stubbed. Verify end-to-end deployed revision if available and acknowledge static/dynamic differences. Obtain independent adversarial review for severe changes with precise code ownership to avoid concurrency overwrite.

## Proof / exit criteria

Final report *must* include exact starting/ending SHA; issues/PRs; before/after p50/p95/p99 + N + host; timeline per bottleneck; root cause vs hypothesis; acceptance matrix statuses `PASS/FAIL/BLOCKED/NOT_RUN`; scripts+commands with exit codes; deployed build identity and browser traces; correctness/security checks; and remaining debt. A goal unmet on cold UI/CLI/Cockpit is FAIL or BLOCKED, never 'done'.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
