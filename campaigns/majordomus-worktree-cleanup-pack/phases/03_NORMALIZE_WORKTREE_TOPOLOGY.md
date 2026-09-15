# Phase 03: Normalize Worktree Topology Without Losing Work

## Goal

Move active work into one deterministic worktree layout and repair stale metadata.

Do not integrate feature branches yet.

## Canonical target

Infer primary repository path `R`.

Infer canonical worktree root as sibling:

```text
parent(R) / (basename(R) + "-wt")
```

For branch `feature/improve-cli`, canonical worktree path should be conceptually:

```text
<worktree-root>/feature/improve-cli
```

Do not hardcode a user home directory.

## 1. Determine protected worktrees

Mark as protected:

- primary integration worktree,
- dirty worktrees until preservation proof is complete,
- worktrees tied to in-progress Git operations,
- worktrees containing unique untracked data,
- worktrees actively required by repository tooling during migration.

## 2. Repair metadata first

Use ordinary Git worktree commands to repair stale entries where possible.

Identify:

- prunable metadata,
- locked worktrees and lock reasons,
- missing directories,
- moved directories,
- duplicate logical worktrees,
- malformed legacy arrangements.

Do not manually delete `.git/worktrees/*` as a first resort.

## 3. Migrate active worktrees

For each active feature branch that should survive:

- ensure branch has one canonical checkout,
- move/recreate its worktree at canonical path,
- preserve file permissions and necessary local ignored state where safe,
- verify Git recognizes the new path,
- verify branch/path bijection,
- verify working tree content matches pre-migration state,
- run lightweight repository sanity checks.

If `git worktree move` is suitable, prefer it.

If recreation is safer, use a preserved branch/ref and compare state before deleting old path.

## 4. Resolve duplicates

If multiple worktrees represent same/superseding feature:

- compare HEADs,
- compare dirty diffs,
- compare untracked state,
- select canonical survivor from semantic evidence,
- preserve unique residue from others,
- remove duplicates only after equality/supersession is proven.

## 5. Naming/path policy

Validate:

- branch name maps to relative path deterministically,
- branch names with characters unsafe for paths are handled by a documented reversible encoding if needed,
- worktree root itself contains no accidental duplicate nesting,
- primary repo is not nested under worktree root,
- worktrees do not live under random temp directories without explicit exception.

## 6. Do not over-normalize history

Topology normalization is filesystem/Git metadata cleanup.

Do not use this phase as an excuse to rebase all branches.

## Output

Provide before/after topology:

```text
branch -> old path -> canonical path -> status -> evidence
```

Record intentionally retained exceptions with reasons.

## Acceptance criteria

- one canonical worktree root exists,
- active feature branches have deterministic paths,
- no duplicate checkout survives without explicit reason,
- stale metadata is repaired or clearly quarantined,
- primary integration worktree remains healthy,
- all migrated work remains byte/commit-equivalent except intentional metadata/path changes,
- `git worktree list --porcelain` accurately reflects filesystem state.
