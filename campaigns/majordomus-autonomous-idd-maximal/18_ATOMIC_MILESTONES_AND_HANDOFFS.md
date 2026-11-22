# Phase 18: Atomic Execution Plan for Multi-Agent Opus/Fable

## Orchestrator of implementation agents
One integration owner controls master/critical domain boundaries. Specialized agents may own non-overlapping scoped changes, but all work must obey mesh board, collision-check, exclusive claims, branch policy, bounded WIP, gates and review.
Agent swarm should not spread eight unfinished infrastructure rewrites at once.

## Atomic milestones
M00 authoritative audit; record current SHA, workflows, source owners, open PRs and live capability map. Exit: evidence-based reuse matrix.
M01 minimal sandbox intent -> reviewed executable plan. Exit: 3-criterion fixture with DAG and plan revision.
M02 persist execution and event replay, authority checks. Exit: restart-stable attempts and documented transitions.
M03 local single-worker executor via existing capability. Exit: one task completes and emits evidence.
M04 mesh-aware scheduling with scope claims. Exit: two workers, one blocked overlap, explainable assignment.
M05 approval and safe pause/resume/cancel. Exit: negative authorization matrix green.
M06 worktree and PR bridge. Exit: sandbox PR created, checked and integrated without duplicate side effects.
M07 version/release/deploy bridge. Exit: sandbox deployed artifact and smoke proof.
M08 derived surface convergence. Exit: one capability changes, generated projections/tests stay consistent.
M09 UI golden path. Exit: full browser flow, mobile and streaming.
M10 .ai governance, docs, site. Exit: generated drift gates and docs source chain pass.
M11 chaos and dogfood. Exit: forced restart, duplicate webhook and failed smoke proven.
M12 guarded rollout. Exit: release tagged/verified only after permissions and evidence.

## Handoff record
Use the existing Majordomus handover/context mechanism, not a second markdown diary.
Minimum content: source SHA; intent/criterion/issue IDs; scope and paths; branch/worktree; event cursor; checks and failing traces; known side effects; PR number; constraints; next safe action and owner.

## Stop conditions
- canonical owner cannot be identified or ownership conflict unresolved;
- proposed operation requires policy approval absent from authorization;
- remote effect state ambiguous and reconciliation cannot settle it safely;
- required checks cannot be verified;
- worktree cleanup threatens unmerged user work;
- production rollout target not explicitly approved;
- prompt pack conflicts with actual repository governance.
Stop the specific unsafe effect, preserve progress and continue independent safe work.

## Reporting per milestone
Status `verified/blocked/failed/partial`, source links, before/after behavior, exact test commands, CI run IDs, commit/PR/release refs, external effect receipts and remaining risk.
Do not mark a milestone complete from the number of lines changed.
