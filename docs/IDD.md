# Intent-driven development

Intent-driven development is the way work is organised in a repository Majordomus
supervises: a person declares what must become true, the plan says what will be done about
it, and the repository derives, on every read, how far the work has come and whether the
intent is satisfied. Nothing about an intent's progress is typed in by hand.

This page is the method in one place. The mechanics are in
[`PLANNING.md`](PLANNING.md#what-the-work-is-for-intents-coverage-and-the-two-planning-records);
the decisions are ADR 0070, 0073, 0075, 0107, 0111 and 0144.

## The records, and who writes each

| Record | Where | Who writes it | What it says |
|---|---|---|---|
| Intent | `.ai/repo/project/intents/<id>.yaml` | a person, or `intents.propose` on a person's word | what must become true, the invariants that must stay true, the milestones that realise it, and the criteria that would satisfy it, each naming the evidence that settles it |
| Gap | `.ai/repo/project/gaps/<id>.yaml` | a person or a worker | what was observed at a commit, and an answer for every criterion: satisfied, missing, conflicting or unknown |
| Critique | `.ai/repo/project/critiques/<id>.yaml` | a person or a worker | the adversarial review of the plan: findings, each blocking or not, each open, planned into an issue, or rejected with a reason |
| Milestone, issue | `.ai/repo/project/milestones/`, `issues/` | a person or a worker | the plan; an issue names the criterion it serves with `serves` |
| Evidence | `.ai/repo/evidence/ledger.json` | the test runner, when a run is recorded ([`EVIDENCE.md`](EVIDENCE.md)) | the latest execution of every test: its outcome, at which commit, on which tree |

Majordomus judges these records. It does not write the gap, the critique or the plan, and it
does not choose what to build.

## What is derived, and from what

An intent stores no status. Every one of these is computed when it is asked for:

| Derived | From | Answered by |
|---|---|---|
| Stage: declared, planned, executing, verifying, satisfied, or cancelled or superseded when the record says so | the status the plan derives for the milestones the intent names, and whether every criterion is met | `majordomus intent list` |
| Verdict: satisfied, unsatisfied, unknown | the criteria alone; closed work does not outrank evidence | `majordomus intent list` |
| Whether a criterion is met | evidence that is current: a passing run in this history, on a clean tree, whose test and whatever it proves have not changed since | `majordomus intent show <id>` |
| Coverage of a criterion: covered, weak, observed, uncovered | the issues whose `serves` names it, and the gap | `majordomus intent coverage` |
| Why an issue exists: intent, maintenance, unexplained | its `serves` and its milestone | `majordomus intent coverage` |
| What a task serves | the issue it is started for, pinned to the task at its start | `majordomus intent binding` |
| What realises an intent, and what work serves nothing | a join of intents, plan, tasks and sessions; every link says whether it was declared, observed, derived or inferred | `majordomus intent realization` |
| Whether work may proceed, and what it is held to | the issue or the paths, the intent they serve, its critique and its governance | `majordomus intent preflight --issue <id>` |

Each is the same answer on the command line, over HTTP under `/api/v1/intents` and as an
MCP tool (`majordomus_intents`, `majordomus_intent_record`, `majordomus_intent_coverage`,
`majordomus_intent_preflight`, `majordomus_intent_binding`, `majordomus_intent_realization`,
`majordomus_intent_explain`), because all three are projections of one declaration.

Two things are not derived yet, and say so rather than guess: a criterion settled by a
`command` or a `deployment` reads `not_derivable` and is never met, and nothing derives a
plan from an intent. Both are recorded as planned claims in
[`PLANNING.md`](PLANNING.md).

## The loop

1. **Declare.** A person writes the intent, or has it proposed: `majordomus intent propose`
   writes a new record and never a stage.
2. **Observe.** The gap records what is already true and what is missing, criterion by
   criterion. An unknown is written as unknown.
3. **Plan.** Milestones and issues are written; each issue names the criterion it serves.
4. **Oppose.** The critique reviews the plan. A blocking finding that is open refuses the
   work it concerns.
5. **Work.** A task is started for an issue and is bound to what the issue serves;
   `majordomus intent preflight` says whether it may proceed and what it is held to.
6. **Prove.** Tests run and their results are recorded as evidence. A criterion is met by
   current evidence and by nothing else.
7. **Read.** Stage, verdict, coverage and realization are asked for, never updated.

`majordomus intent validate` and the `intent-check` gate refuse a model that does not hold
together: a criterion with no work, an issue under an intent's milestone that serves
nothing, work executing without a critique or with an open blocker.

## Where to read it

| Surface | Where | What it is |
|---|---|---|
| Command line | `majordomus intent list`, `show`, `coverage`, `preflight`, `binding`, `realization`, `explain`, `validate` | the live answer in this checkout |
| HTTP | `GET /api/v1/intents` and the routes beside it | the same answer, served |
| MCP | the `majordomus_intent*` tools | the same answer, to an agent |
| UI | `/ui/roadmap?intent=<id>` | the intent beside the milestones that realise it |
| Cockpit | `/cockpit/intents`, `/cockpit/intents/<id>` | the realization: what serves it and what serves nothing |
| This site | `/intents/` and a page per intent | what the records and the plan decide |

The published pages show the statement, the invariants, the milestones with the status the
plan derives for each, and every criterion with the evidence that settles it. They do not
show which criteria are met: evidence goes stale when a file its test names changes, so a
static page would be right on the commit it was built from and wrong on the next. The live
answer is the command.

## In another repository

Nothing above is specific to this repository. A repository Majordomus supervises declares
an intent by adding a file under `.ai/repo/project/intents/` that validates against
`share/schemas/majordomus/intent/intent.v1.schema.json`; the same commands, routes and tools
then answer for it. A repository that declares none has none: every surface answers with an
empty list, and no work is refused for lacking an intent unless its policy sets
`intent.binding` to `required`.
