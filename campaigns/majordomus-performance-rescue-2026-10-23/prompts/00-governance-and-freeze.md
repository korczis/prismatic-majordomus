# Phase 00: Governance, source truth, and scope freeze

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 00 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Read AGENTS.md and .ai/README.md, all applicable hierarchy, the effective rules and task-lifecycle workflow before touching code. Identify the current SHA/branch, tracked/dirty state, linked worktrees, running session/mesh board, existing performance PRs and the maintained issue graph. Announce scope through the project-supported interface, do not claim exclusivity unless acquired. Inspect `I3226`, `I3442`, `I3299`, `I3227`, `I3140`, and completed `I2486`, `I2489` from their canonical `.ai/repo/project/issues/*.yaml` records and GitHub. Find related PRs and branches before creating any. Revalidate the master prompt's leads against current HEAD; their measured historical values are not this run's baseline. Explain which existing rule/gate covers each symptom and where it doesn't. Determine exact shell vs Rust CLI entry points and how commands are forwarded, not by guessing from names.

Construct a *minimal performance train* rooted in the owner complaint with one integration owner and clear dependency/scope partitions. Repair within existing issues where permitted; if no canonical P0 owner exists, create exactly one issue linked to the existing graph and mark child tasks or reuse existing issues, never fork competing issue trees. Do not add a new front-end, service, store, scheduler or global dashboard. Keep release train isolated from simultaneous mesh work and do not disrupt active worktrees.

## Proof / exit criteria

Output an evidence-backed `BASELINE-SCOPE.md` with current SHA, canonical issues/PRs, active work claims, scope ownership, environment/tool availability, concrete command routes, risk register and a single authoritative train path. Gate: no implementation until current tree and overlap are understood; absent access is BLOCKED with explanation.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
