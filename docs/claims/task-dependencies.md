# A task record has no dependencies; sequencing between sessions is not represented

## What it means

**Rejected, and narrowly.** A task is the record of one session's work — what a worker
declared, in which checkout, under which profile. No field, file or command says task B waits
on task A. One task is active per checkout, and coordination between checkouts is the scope
overlap report, not an ordering.

This is not a statement that the repository has no dependency graph. It has two. Issues carry
`depends_on` and are validated as a DAG with derived execution waves; milestones carry
`depends_on` above them, and a milestone whose dependencies are not accepted is blocked. Both
are described in [`PLANNING.md`](../PLANNING.md) and [`ROADMAP.md`](../ROADMAP.md).

The refusal is about the *task* record specifically, which is a different noun from an issue:
an issue is the contract for a piece of work, a task is one session's attempt at it.

## How it works

Nothing is implemented, on purpose. A task declares a scope; overlap between concurrent
checkouts is computed from git worktrees and reported. Sequencing between sessions is a human
decision, or it is expressed one level up as a dependency between the issues those sessions
are executing.

## How to see it

```bash
grep -rn depend .majordomus/state/current.yaml     # nothing to find
majordomus plan graph                              # the issue graph, which does exist
majordomus plan rgraph                             # the milestone graph, which also does
```

## What it does not cover

It says nothing about the issue or milestone graphs, which are the supported way to express
that one piece of work waits on another. A team wanting sequencing should put it there, where
it is validated, rather than wanting it on the task record, where it is refused.

## Why it exists

No use case in the evidence justified dependencies *between task records*. The one task record
with dependency-like fields found in the source environment was a demonstration file whose
subtasks were still pending a year later.

The wording was previously "Dependencies between tasks are deliberately not represented", with
a note saying no dependency graph existed and would be revisited when a real case appeared.
The real case appeared: the plan model landed, and with it two validated graphs. The claim
itself never became false — a task still has no dependencies — but a reader meeting it beside
`/plan/dag/` would reasonably have concluded the page was stale. Narrowed to say what is
actually refused rather than to imply the absence of something that now exists.
