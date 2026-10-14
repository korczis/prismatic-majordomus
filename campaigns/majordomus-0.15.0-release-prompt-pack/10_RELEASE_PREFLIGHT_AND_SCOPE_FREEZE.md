# Milestone 1 — Release preflight and scope freeze

Goal: establish exact 0.15.0 release contract from current repository reality and prevent scope creep.

1. Load Majordomus context/rules/workflows first.
2. Inspect current master, PR #794, overlapping PRs, current plan/issues/intents, current release baseline.
3. Run peer/collision/worktree checks required by policy.
4. Inspect actual implementation for verdict, freshness, preflight, cross-surface projection, intent CI, GitHub sync hardening and release machinery.
5. Create canonical release scope table:

```text
item | current state | classification | evidence | canonical owner/source
```

Classifications only: MUST SHIP, BLOCKER FIX, DEFER, OUT OF SCOPE.

Do not mutate architecture except for a tiny correction required to complete preflight.

Acceptance:
- current master SHA known;
- candidate SHA known;
- merge-base/ahead/behind/conflict state known;
- overlap state known;
- release baseline known;
- scope frozen;
- next-wave ideas deferred;
- canonical task/session passes preflight.

Output exact refs, scope table, blocker list, expected paths, next command.
