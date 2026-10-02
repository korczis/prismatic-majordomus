# Phase 01: Forensic Repository, Branch, and Worktree Inventory

## Goal

Build a complete, evidence-backed picture of repository state before touching it.

This phase is read-mostly. Do not normalize, delete, rebase, or merge yet.

## Required inspection

Discover canonical repository root and current integration branch.

Collect and correlate at least:

- `git status --porcelain=v2 --branch` in primary repo and every worktree,
- `git worktree list --porcelain`,
- `git branch -vv`,
- `git for-each-ref` for local and remote refs with upstream/object metadata,
- `git remote -v`,
- fetch state and remote default branch,
- current HEADs and detached HEADs,
- merge bases against `origin/master` or canonical integration branch,
- ahead/behind counts,
- unique commits per branch,
- branches already merged,
- branches whose patch content is equivalent despite different history,
- in-progress merge/rebase/cherry-pick/revert/bisect state,
- stash inventory,
- reflogs relevant to recent work,
- dangling/unreachable commits via safe inspection,
- worktree administrative metadata,
- filesystem worktree roots outside the canonical sibling `-wt` location,
- worktrees that exist on disk but not in Git metadata,
- Git metadata entries whose directories no longer exist,
- duplicate checkout attempts for same conceptual feature,
- branch naming/path mismatches,
- worktrees with untracked files not represented in commits.

Fetch remote refs safely before computing final ahead/behind values.

Use prune-aware fetch if repository policy permits, but do not delete local branches in this phase.

## Semantic inspection

For every non-integration branch, inspect enough history/diff to determine:

- what feature/refactor/fix it appears to contain,
- whether it overlaps another branch,
- whether another branch supersedes it,
- whether it contains generated-only noise,
- whether docs/tests reveal intended behavior,
- whether commit messages reference issues/prompts/sessions/handoffs,
- whether it is likely safe to rebase,
- whether it has public/shared upstream history.

Do not rely only on branch names. Humans name branches under deadline pressure, a known source of entropy.

## Repository clutter inventory

Also identify non-Git topology clutter that may interact with cleanup:

- root scripts that duplicate canonical tooling,
- temporary patch files,
- abandoned merge reports,
- duplicate session-context/handover locations,
- stale generated artifacts,
- legacy worktree manifests,
- obsolete local bootstrap files,
- directories that appear superseded by `.ai/**` or `.majordomus/**`,
- committed caches/build artifacts,
- ignored but unusually large or important-looking files.

Do not delete anything yet.

## Output

Produce a structured campaign inventory with one row/object per worktree/branch.

Required fields:

```text
branch
worktree_path
head_oid
upstream
remote_presence
cleanliness
staged_changes
unstaged_changes
untracked_count
ignored_risk
merge_base_with_integration
unique_commits
patch_equivalence_notes
likely_dependencies
shared_history_risk
classification
recommended_next_action
confidence
evidence
```

Also produce separate sections for:

- stashes,
- detached HEADs,
- dangling/reflog-only commits,
- filesystem-only worktree directories,
- metadata-only worktrees,
- clutter candidates.

## Classification vocabulary

Use a disciplined classification rather than prose soup:

- `integration-base`
- `ready-independent`
- `depends-on:<branch>`
- `overlaps:<branch>`
- `superseded-by:<branch>`
- `already-integrated`
- `dirty-needs-preservation`
- `detached-needs-recovery`
- `shared-history-rewrite-risk`
- `abandoned-but-unique`
- `stale-no-unique-work`
- `unknown-block-delete`

Multiple labels are allowed.

## Acceptance criteria

Do not proceed until:

- every worktree is accounted for,
- every local branch is accounted for,
- every remotely tracked feature branch relevant to current work is accounted for,
- dirty/untracked state is known,
- stash state is known,
- detached/unreachable state has been inspected,
- obvious branch dependencies/overlaps have initial evidence,
- canonical topology violations are enumerated,
- no destructive action has occurred.
