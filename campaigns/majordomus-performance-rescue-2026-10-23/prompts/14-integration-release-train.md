# Phase 14: Integrate measured repairs as one release train

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 14 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Use the repository's canonical IDD, .ai task lifecycle, GitHub issue/PR/milestone/wave and mesh board conventions. Select one integration owner/branch and narrow child branches only for truly independent scopes. Coordinate overlapping I3226 timing, I3442 plan flight and any shared executor/readings edits with active workers; hand off explicitly rather than racing. Implement atomic, reviewable commits for instrumentation, cold UI repair, CLI entry, selective warming, expensive projection caching, concurrency/resource, browser regression and docs. Rebase/merge against live master as governed by project, resolve generated-file drift by regenerating from merged tree once, not copying generated outputs across branches.

Keep tests near each patch. Run `majordomus usecase impact` or actual supported equivalents; retain backward compatibility for CLI commands, public routes, JSON/MCP/OpenAPI names. Refuse a merge if performance proof, tests, browser evidence or security verdict is missing. Inspect open PRs every integration step, don't create N disconnected performance trains or run long agent swarms without ownership.

## Proof / exit criteria

Output train manifest linking issue -> commit -> test -> benchmark -> docs -> deploy. Close issues only where evidence proves their acceptance criteria. A merged train must have an exact SHA and CI verdict; if blocked, leave a merge-ready branch/PR with honest reasons and all completed work intact.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
