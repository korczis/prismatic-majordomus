# Phase 09: Make the Clean Topology the Enforced Default

## Goal

Prevent recurrence by turning worktree conventions into validated repository behavior.

Do not create a second worktree subsystem if one already exists. Extend canonical Majordomus tooling.

## 1. Audit existing worktree support

Search for:

- worktree commands,
- branch creation helpers,
- `.envrc` integration,
- Just recipes,
- Rust CLI modules,
- API routes,
- MCP tools/resources,
- Cockpit views,
- rules/doctrines,
- schema/front matter describing worktrees,
- Git hooks/gates,
- docs.

Identify duplication and gaps.

## 2. Canonical typed inventory

Where permanent tooling is justified, create or reuse one typed representation derived from Git and repository policy.

Conceptually it should represent:

```text
repository root
canonical worktree root
integration branch
worktrees[]
  branch
  path
  head
  upstream
  clean/dirty state
  ahead/behind
  canonical-path-valid
  stale/merged indicators
  diagnostics
```

Do not persist a hand-maintained list of worktrees.

Git remains canonical operational state; Majordomus derives/validates it.

## 3. Lifecycle operations

Prefer a small coherent command set integrated into existing CLI conventions, for example conceptually:

```text
majordomus worktree list
majordomus worktree status
majordomus worktree doctor
majordomus worktree create <branch>
majordomus worktree plan-clean
majordomus worktree clean --apply <plan-id-or-equivalent>
```

Do not use these exact names if repository already has an established namespace.

Critical behavior:

- create derives canonical path automatically,
- validation rejects duplicate/ad-hoc active topology,
- cleanup defaults to plan/dry-run,
- destructive cleanup requires explicit intent,
- dirty/untracked work blocks deletion unless preservation is explicit,
- JSON output is available through canonical structured-output patterns,
- operations are deterministic and testable.

## 4. Rule/doctrine

Encode at least these invariants in the proper governance mechanism:

> Active feature worktrees live under the inferred sibling `<repo>-wt` root and map deterministically to their branches.

> A feature branch has at most one canonical active worktree.

> Destructive worktree cleanup is forbidden when unique tracked, untracked, detached, or stashed work has not been preserved and classified.

> Worktree/branch inventories are derived from Git and canonical repository policy; consumers may not maintain independent registries.

> Integration into the canonical branch follows validated repository gates and cannot be force-pushed by cleanup tooling.

These must be executable/enforced, not decorative prose.

## 5. Hooks/gates

Integrate fast topology validation into appropriate local/CI gates without making every shell command miserable.

Possible checks:

- worktree path mapping,
- duplicate branch checkout,
- primary integration branch location,
- stale metadata diagnostics,
- forbidden ad-hoc worktree roots,
- canonical branch naming where repository policy defines it.

Do not fail CI for developer-local worktrees that CI cannot observe unless the rule is explicitly scoped correctly.

## 6. Tests

Add tests for:

- canonical root derivation,
- nested branch names,
- duplicate worktrees,
- detached worktree classification,
- dirty cleanup refusal,
- untracked cleanup refusal,
- dry-run plan determinism,
- merged branch detection,
- path mismatch diagnostics,
- worktree create/remove lifecycle in temporary repositories,
- no data loss on refused destructive operations.

Use integration tests with temporary Git repos where appropriate.

## Acceptance criteria

- topology conventions are executable policy,
- normal worktree creation automatically does the right thing,
- unsafe cleanup is blocked,
- one canonical inventory feeds all consumers,
- regression tests exist,
- legacy scripts/duplicate registries in scope are removed.
