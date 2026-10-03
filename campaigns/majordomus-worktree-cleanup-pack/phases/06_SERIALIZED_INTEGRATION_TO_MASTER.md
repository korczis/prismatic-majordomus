# Phase 06: Serialized Integration Into the Canonical Integration Branch

## Goal

Integrate verified work one unit at a time, keeping `master` or equivalent continuously understandable and testable.

## 1. Prepare integration worktree

Use the primary repository root for the canonical integration branch unless repository policy dictates a dedicated integration worktree.

Verify:

- clean status,
- exact match/relationship with `origin/master`,
- no hidden local-only commits,
- no in-progress Git operation,
- current remote fetch is fresh.

If remote advanced since DAG planning, update/recompute affected branches rather than integrating against a stale base.

## 2. Integrate one DAG node at a time

For each ready unit:

- verify its base relationship,
- use repository-approved fast-forward/merge strategy,
- avoid unnecessary merge commits if policy prefers linear history,
- preserve meaningful commit boundaries,
- do not batch unrelated branches into one merge.

After each integration:

- inspect resulting diff/history,
- run focused tests,
- run shared gates likely affected,
- ensure generated outputs remain canonical,
- ensure worktree topology remains valid,
- record new integration branch OID.

## 3. Rebase downstream nodes as integration base advances

When a downstream branch depends on newly integrated work, update it against the new base before merging.

Do not rely on an old rebase from before earlier nodes landed.

This keeps each merge a deliberate proof against the actual final base.

## 4. Full gates at logical milestones

After coherent batches, run broader repository gates.

At minimum before final push:

- format checks,
- workspace build/check,
- full relevant tests,
- CLI tests,
- API/OpenAPI tests if present,
- MCP tests if present,
- Cockpit/frontend tests if touched,
- docs/site build,
- schema/front matter validation,
- generated artifact drift checks,
- worktree lifecycle validation.

Use canonical repo commands such as `just check` only if they actually exist and are canonical.

## 5. Push policy

Push integration branch only when current integrated state passes required gates.

Never force-push integration branch.

If remote moved concurrently:

- fetch,
- stop push,
- re-evaluate/rebase integration safely according to policy,
- rerun affected tests.

Do not resolve race conditions by overwriting remote history.

## 6. Mark merged branches

After each successful integration, record:

- old feature HEAD,
- integrated commit/range,
- resulting master OID,
- whether branch is now fully merged/equivalent,
- whether cleanup is safe in phase 08.

Do not delete branches/worktrees immediately unless repository workflow requires it. Cleanup is a separate verified phase.

## Acceptance criteria

- every planned merge-worthy unit is integrated or explicitly deferred,
- integration order followed the DAG,
- no broken intermediate state was pushed,
- final local integration branch passes full gates,
- `origin/master` synchronization is verified,
- cleanup candidates are identified by exact evidence.
