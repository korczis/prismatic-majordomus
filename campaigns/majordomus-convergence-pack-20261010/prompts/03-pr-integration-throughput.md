# Milestone 03: Existing PR integration engine to fast safe throughput

ADR 0101 and `crate::integration` (verify current locations), `prs refresh`, `prs status`, `prs drain --dry-run`, `prs drain --max`, SHA cache, leases, rank and disposition classification ALREADY EXIST. Do not create another PR merger, cron orchestration or state machine. Begin with current live PR census from Milestone 00. Merge ONLY if project permissions and preexisting policy authorize, never bypass red checks or unresolved conflicts. `github mergeable` is not reliable with custom derived merge driver; inspect local merge-tree semantics.

## Issue 03.1: Backlog hygiene, evidence-based

Map open PRs to current master, issue scope, ownership, impact, check readiness and supersession. For stale PRs choose refresh/rebase, integrate into a smaller coherent PR, hold, or close only with evidence. Report risk and owners. Respect user work and never force-push/delete other's branch automatically. Repair duplicate derived changes through existing merge driver.

## Issue 03.2: Optimize observation and planning

Instrument time and network calls of refresh/classification/merge-tree and CI polling. Cache immutable computations by precise (base SHA, head SHA, driver/config version), invalidate after every merge. Batch independent read operations with rate-limit awareness; process critical readiness first without dropping any open PR from the census. Ensure deterministic rank and fairness/anti-starvation. Keep observable reason strings.

## Issue 03.3: Safe incremental drain and recovery

One verified merge against the *current* master at a time; after merge discard old plan and re-observe. Bounded refresh/rebase; provide no-op for no safe PR. Crash-resistant lease, idempotent restart and replay audit; enforce branch protection and required checks. Do not relax safety to chase throughput. Preserve fail-closed behavior when GitHub/CI cannot be read.

## Issue 03.4: Cross-worktree coordination

Use existing session scopes and plan-wave conflict model. Identify stale worktrees and duplicate patches without modifying them by default. Allow parallel non-conflicting agent work and single final integration lane. Handle output conflicts and shared generated files distinctly from source-code conflicts. Prove no lost edits through simulated overlapping worktrees.

## Issue 03.5: Tests and measured rollout

Integration fixtures: 50+ PR mock census, already-merged, superseded, red checks, ahead/behind, conflict via merge drivers, stale branch, changed master between observe/act, lost network, abuse of lease, duplicate merge, crash, protected master, no protected master. Document before/after time to integrate eligible PR, throughput per hour and intervention frequency. Execute first real merge only with required existing authorization and green evidence; otherwise dry-run and report blockers.

Acceptance: backlog drains incrementally and deterministically without losing work, requiring manual help only on genuine blockers. No wholesale rebases, no surprise force pushes, no new parallel integrator.
