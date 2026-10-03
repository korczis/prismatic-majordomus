---
id: github-workgraph-phase-05
phase: 5
depends_on: [github-workgraph-phase-04]
goal: tri-directional Git/worktree/commit/PR traceability
---

# Phase 05 — Git, Worktree, Commit, PR, and Evidence Traceability

Read shared contract and prior handovers.

## Objective

Make "milestone → issue → code" and "code → issue → milestone" mechanically true.

Integrate the existing canonical worktree topology rather than inventing another branch/workspace system.

## Required relations

For a managed execution contract, derive or record according to policy:

- canonical branch;
- canonical worktree path;
- active session/claim if current session system supports it;
- relevant commit set/range;
- PR(s);
- merge status;
- checks;
- changed paths;
- acceptance evidence.

Reverse queries must work:
- branch → work item;
- worktree → work item;
- commit → work item(s) or explicit ambiguity;
- PR → work item(s);
- check/evidence → PR/work item;
- release commit/tag → contributing accepted work items/outcomes where derivable.

## Association rules

Do not assume commit messages always contain issue numbers.

Prefer a hierarchy of reliable signals based on current architecture:
1. canonical worktree/branch ownership;
2. explicit typed mapping/projection;
3. PR head/base relationship;
4. commit ancestry/range;
5. canonical metadata markers if used;
6. commit-message references only as supporting hints unless doctrine explicitly makes them authoritative.

Ambiguity must be represented and surfaced. Do not "guess" a single issue from weak evidence.

## Worktree enforcement

Reuse the existing worktree subsystem and rule such as `project.worktree-topology`.

Strengthen it so managed feature work can be validated against the Work Graph:

- active managed issue with implementation scope should have its expected branch/worktree when policy requires;
- managed worktree should map to a work item or explicit allowed exception;
- mismatched branch/worktree should fail with migration/fix instructions;
- orphan worktree/branch should be visible;
- duplicate active ownership should be detected where not allowed.

Do not break legitimate maintenance/spike/release branches; model explicit exceptions.

## Commit attribution

Implement deterministic attribution based on Git topology.

Consider:
- branch fork point;
- rebases;
- squash merge;
- merge commits;
- cherry-picks;
- multiple PRs;
- commits shared by branches;
- amended history.

Separate:
- "commit was authored while executing issue X";
- "commit is reachable from branch X";
- "commit landed in PR X";
- "commit is now reachable from trunk".

Do not collapse these into one boolean.

## PR mapping

Map GitHub PR observations to canonical work:
- head/base refs;
- external ID/number;
- linked issue(s);
- commits;
- checks/reviews;
- merged commit;
- state.

If one PR serves multiple issues, support it explicitly or reject it according to documented policy. Do not silently lose many-to-many relationships.

## Acceptance evidence

Convert suitable CI/check information to typed evidence only through policy:
- which check names/capabilities count;
- freshness;
- target SHA;
- required vs optional;
- local vs GitHub verification.

A green check on the wrong SHA must not satisfy the contract.

## Completion proof

Implement a query that can explain:

`Issue X is complete because: dependencies A/B accepted; PR #N merged at SHA S; required checks C/D passed on S; documentation/generation gates passed; acceptance criteria K1..Kn each have evidence.`

And equally:

`Issue X is NOT complete because criterion K3 lacks evidence and GitHub is closed prematurely.`

## Tests

Create temporary Git repositories/fixtures for:
- normal feature branch;
- canonical worktree;
- rebase;
- squash merge;
- merge commit;
- cherry-pick ambiguity;
- orphan branch;
- wrong worktree;
- PR merged but missing required check;
- check green on stale SHA.

## Gate

The reverse traversal from commit/PR to canonical issue/outcome must be functional and tested, not merely represented in docs.
