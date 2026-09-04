# Planning — milestones, issues, and the graph between them

This document explains what the model means. It contains no current figures: what is true
right now is printed by `majordomus plan status` and rendered on the website's roadmap from
the same data. If you want to know how many issues are ready, run the command.

`majordomus start` supervises one task. This layer supervises the thing a task belongs to.

## The problem

A task record says what one session is doing. It does not say why that work exists, what it
depends on, what would prove it finished, or what should be done after it. That knowledge
lived in conversation, and conversation does not survive a session, a provider switch, or a
handover to a person.

Three failures follow. Work gets picked from memory rather than from a dependency order, so
something is started before the thing it needs. "Done" gets asserted rather than shown.
And the plan exists in several places at once — a chat, a README paragraph, a hand-drawn
diagram, a GitHub milestone — which disagree the first time one of them changes.

## The two records

A **milestone** is an executable specification of an outcome. Not a folder for tickets: a
statement of the problem, the outcome that ends it, the current and desired state, what is in
scope and what is deliberately not, the criteria that would make it true, the validation that
would demonstrate it, and the evidence required to accept it. One file under
`.majordomus/project/milestones/`.

An **issue** is a bounded execution contract. It carries enough for a worker with no
conversation history to execute it: the objective, why it exists, the current and desired
state, the paths it may touch, what it depends on, the acceptance criteria, the validation
commands, and the evidence its completion requires. One file under
`.majordomus/project/issues/`.

Both are hand-written YAML in the same restricted subset the policy uses, and both are
checked against an allowlist: a key nobody reads is an error, not a comment.

An issue names its milestone. A milestone does not list its issues. One direction means the
two can never disagree about which issues belong to which outcome.

## Status is derived

Neither record has a `status` field. Writing one is an unknown key and fails validation.

An issue records what happened to it — `started_at`, `verified_at`, `completed_at`,
`cancelled`, and its `evidence` — and the status follows:

| status | when |
|---|---|
| `CANCELLED` | `cancelled: true` |
| `DONE` | `completed_at` is set and every `evidence_required` token has evidence |
| `VERIFY` | implementation is claimed complete and the evidence is not yet sufficient |
| `ACTIVE` | `started_at` is set and nothing further is recorded |
| `BLOCKED` | a dependency is not `DONE` |
| `READY` | nothing above applies |

A milestone's status follows from its issues and its own evidence:

| status | when |
|---|---|
| `PLANNED` | it has no issues, or none of them has moved |
| `ACTIVE` | at least one issue is active, verifying or done |
| `BLOCKED` | it is unfinished and no issue is ready |
| `VERIFY` | every issue that is not cancelled is done and the milestone's own evidence is not complete |
| `DONE` | that evidence is complete too |

A milestone is never `DONE` because a count of closed issues reached its total. The last step
is the milestone's own acceptance, and it is evidence-gated like every other step.

The **active milestone** is derived the same way: the lowest-ordered milestone that is
`ACTIVE`, or failing that the lowest-ordered one that is not finished. Nothing declares it.

## The graph

`depends_on` is a list of issue ids. The graph they form is validated, not trusted. Each of
these is a distinct finding naming the issue that caused it:

- a dependency on an issue that does not exist
- an issue that depends on itself
- the same dependency named twice
- an issue naming a milestone that does not exist
- a cycle, with every issue trapped in or behind it named
- an issue that is `ACTIVE`, `VERIFY` or `DONE` while a dependency is not `DONE`
- an issue with no acceptance criteria or no validation command — a placeholder

A failing finding makes `majordomus plan validate` and `majordomus doctor` exit non-zero.
`majordomus watch` reports the same violations as drift. Work in progress is reported and
never blocked: an issue that is merely unfinished is not a failure of anything.

## Execution waves

A **wave** is a layer of the graph. Wave zero is every issue with no dependencies; an issue
sits one layer past its deepest dependency. Waves are computed on every read and stored
nowhere, so an execution plan cannot go stale — changing one edge moves every wave that
depends on it, immediately, on every surface.

Sharing a wave is necessary for two issues to run concurrently. It is not sufficient. If
their declared `scope` paths overlap — one equal to, inside, or containing another — the
overlap is reported and they serialise. The direction is deliberate: a false serialisation
costs time, a false parallel costs a conflict discovered after the work is done.

## Evidence

An issue declares `evidence_required` as a list of tokens. `majordomus plan evidence`
attaches one piece of evidence against one token and refuses a token the issue does not
declare. It also refuses without a command or an artifact — narrative is not evidence.

`majordomus plan done` refuses while any token is uncovered. An issue whose `completed_at`
is set but whose evidence is incomplete derives `VERIFY`, never `DONE`. The evidence lives in
the issue's own file, beside the contract it satisfies, with the commit it was recorded at.

What this does not do: rerun the command. The tool records what a worker says a command
produced. The commit hash stored beside it is what makes a false record checkable later.

## Projections

The canonical files are the only source. Everything else is generated from them by one
engine — `lib/project.awk`, loaded by `lib/project.sh` — so no two surfaces can hold
different opinions about what is ready:

| surface | how |
|---|---|
| the command line | `majordomus plan` |
| the Mermaid DAG | `majordomus plan graph` |
| GitHub milestones and issues | `scripts/github-sync` |
| the website's roadmap, milestone, issue and DAG pages | `scripts/generate-site-data` |
| the documentation | this file explains the semantics; the figures are generated |

GitHub is a projection and a place to talk, never the source. A canonical change updates the
generated region of an issue body; a person editing that region is reported as drift and not
overwritten; a person's comments and any text outside the region are never touched. Nothing
is read back: closing an issue on GitHub does not complete it here.

The network calls live in `scripts/github-sync`, outside the tool. `bin/`, `lib/`, `share/`
and `test/` contain no network client, and `test/cases/08_no_forbidden_constructs.sh` proves
it.

## Working through the model

```bash
majordomus plan status                  # where the outcome stands
majordomus plan next                    # the one issue to take now
majordomus plan show I0007              # the whole contract, no chat history needed
majordomus plan start I0007             # refused unless it is READY
# ... execute, inside the scope the issue declares ...
majordomus plan verify I0007            # implementation complete, evidence pending
majordomus plan evidence I0007 --covers doctrine_test --type test \
  --command "bash test/run.sh 44_model_doctrine" --result "1 passed"
majordomus plan done I0007              # refused while a required token is uncovered
majordomus plan next                    # recomputed, not chosen
```

A worker takes the issue the graph offers. Picking something else needs a reason: it is a
blocker, it invalidates an assumption the milestone rests on, or a person reprioritised.

## Replanning

The plan is expected to change. Add an issue, remove one, split one, merge two, move an edge,
narrow a scope, change a validation — all of it is editing the canonical files, and all of it
is re-validated on the next read. Two constraints survive every edit: the graph stays acyclic
and resolvable, and evidence already recorded is not deleted to make a story tidier.

Replanning is not optional when an issue discovers a dependency nobody predicted, an
acceptance criterion turns out to be untestable, an implementation path is disproven, or an
issue becomes unnecessary. Leaving the graph describing a plan that reality has left behind
is the failure this layer exists to prevent.

## What was rejected

**A `status` field with validation.** Two sources of truth and a rule to reconcile them. The
graph is the only thing that can be right, so it is the only thing that is stored.

**A milestone that lists its issues.** A second edge, in the opposite direction, that can
disagree with the first.

**A separate evidence store.** The contract and the proof that it was met belong in one
record; splitting them lets one be read without the other.

**A mapping file from canonical id to GitHub number.** State that has to be kept in step with
two systems. The id prefixes the title instead, and matching is a string comparison.

**Estimates, velocity, burndown.** None of them changes what a worker should do next, which
is the only question this layer answers.

**`PLANNED` as an issue status.** It would only ever mean `READY` or `BLOCKED` with a
different word on it.

## See also

- [`docs/DOGFOODING.md`](DOGFOODING.md) — why this repository uses the model on itself
- [`docs/SCHEMAS.md`](SCHEMAS.md) — the fields of each file
- [`docs/CLI.md`](CLI.md) — `majordomus plan`, subcommand by subcommand
- [`docs/DOCTRINE.md`](DOCTRINE.md) — how `project_integrity` and `dag_integrity` are enforced
