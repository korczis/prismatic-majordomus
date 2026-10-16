# Mission 05 — 42-PR queue reconciliation and safe landing

Historical baseline: GitHub search returned 42 open PR on Oct 14 2026; recalculate fully. Some old stacked PR descriptions list #922, already merged at observed HEAD. Never treat descriptions as actual current diff.

## Algorithm

1. Snapshot each open PR: number/title, head SHA, base SHA, merge base, stack relationships, changed paths, patch-id equivalents, checks, reviews, comments, linked issues, CI costs, conflicting resource claims, artifact compatibility, age, latest activity.
2. For each PR compare diff against live master: classify `unique`, `patch-present-in-master`, `partially-present`, `superseded`, `stale-target`, `unsafe`, `blocked-by-failing-check`, `externally-blocked`. Use `git range-diff`, `patch-id`, file-level comparison and tests. Never classify from title alone.
3. Build DAG of prerequisites and file conflicts, with canonical deterministic ordering. Group minimal batches by independent ownership, not PR number or age. Identify hidden duplicate work and generation-only diffs.
4. For patches fully present and proven on trunk, close PR with precise evidence and issue state reconciliation (owner approval policy where required). Don't silently discard unique content.
5. For partial patches, rebase to trunk safely, split/cherry-pick unique behavior into clean scoped branch after deriving projected files, preserve attribution and author history where possible. Never force push unowned branch without explicit authority and lease.
6. Before landing: validate changes at new SHA; wait for required CI on EXACT SHA, detect failed tests, do not pass via `--admin` exemption. If trunk red, only designated red-trunk recovery carrier may land until green by policy.
7. Merge through repository's documented merge/train mechanism; no wholesale force, no blanket auto-merge, avoid simultaneous overlapping files; respect provider and GitHub limits.
8. Close issue only when accepted outcome proven by landed commit and evidence pointers. Split deferred work to correctly typed issues with reason, not generic TODO.
9. After landing, derive/run gate, check branch cleanup safety, update generated docs/pages only as real deployment requires. Never delete worktrees with unmerged or unowned changes.

## Queue metrics

Track PRs_open, unique_patch_sets, redundant_PRS, merge_conflicts, CI-blocked, median/p95 age, time_to_merge, P0/P1 landing lead time, red-trunk interruptions, aborts. All with timestamp/window and computed denominator.

Acceptance: decreasing genuine unique integration backlog with no lost patches or bypassed checks. Target zero open PR only when this accurately reflects done work; no invented success through closure. Output per-PR resolution ledger with link to actual result.
