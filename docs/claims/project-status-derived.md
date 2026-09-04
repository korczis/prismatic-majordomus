# Issue and milestone status is derived from recorded facts and is stored nowhere

## What it means

There is no `status:` field in an issue or a milestone. Writing one is an unknown key. An issue records what happened to it — `started_at`, `verified_at`, `completed_at`, `cancelled`, and its `evidence` — and `BLOCKED`, `READY`, `ACTIVE`, `VERIFY`, `DONE` or `CANCELLED` follows from those facts together with the state of the issues it depends on. A milestone's status follows from its issues and its own evidence.

## How it works

`lib/project.awk` computes, for each issue, whether it is locally done (completed, not cancelled, every required evidence token covered), then whether any dependency is not done. The precedence is cancelled, done, verify, active, blocked, ready. A milestone is `DONE` only when every issue that is not cancelled is `DONE` **and** its own `evidence_required` is covered; when the issues are all done and the milestone's evidence is not, it is `VERIFY`. The active milestone is the lowest-ordered milestone that is `ACTIVE`, or failing that the lowest-ordered one that is not finished.

## How to see it

```bash
majordomus plan list
# I0002   BLOCKED   1    M000   Define the canonical milestone and issue schema
majordomus plan done I0001
# plan: I0001 DONE
# next ready issue: I0002
```

## What it does not cover

Derivation cannot tell you that a recorded fact is false. If `completed_at` is set on an issue whose work was never done, the graph believes it — which is why `plan done` refuses without evidence, and why the evidence has to be a command.

## Why it exists

A stored status is a second opinion. The moment it can disagree with the dependency graph, somebody has to decide which one is right, and the answer is always the graph.
