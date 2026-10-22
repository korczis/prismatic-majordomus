# The roadmap — milestones as a graph

[`PLANNING.md`](PLANNING.md) describes the two canonical records, how status is derived,
how evidence gates completion, and how the model is projected. This document covers the
layer above that: the graph *between* milestones, which decides which outcomes are
reachable at all.

The roadmap is not a document. It is a projection of milestone state.

```
majordomus plan roadmap        # the sequence, with now and next
majordomus plan rgraph         # the same graph as Mermaid
majordomus plan roadmap --json # the same, for a generator
```

Nothing in this file is a roadmap. Run the command.

## Two graphs, deliberately not one

The **work graph** is issue to issue inside a milestone: what may a worker execute next.
It is the subject of `PLANNING.md`.

The **roadmap graph** is milestone to milestone: which outcomes are reachable, and in what
order. Its edges are a milestone's `depends_on`.

They stay separate because they answer different questions for different readers.
Collapsing them would make "what should I do this morning" and "what are we building this
year" the same query.

## Identity is a slug; a version is a release that happened

A milestone is named by its slug, and nothing else. Its page URL, a claim's link to it and its
GitHub binding all key on that slug.

A milestone does not carry a version (ADR 0125). Versions used to be a `version:` field typed into
each record, `0.1` to `1.0`. They named releases that did not exist yet, two milestones said
`0.5`, and the tool shipped `0.18.0` while the page still read `0.6`. A version is a published
release record, and the crate manifest is the one place a version is authored (ADR 0085). The
field is still accepted, so an adopter's records keep validating, but nothing reads it.

Nothing may infer a record's kind from the shape of its id. `mj_pj_is_milestone` asks the
loaded model, because a milestone id is a slug and an issue id is not distinguishable from
one by spelling.

## Horizons, releases and what is inferred

`plan.roadmap` answers with an `inferred` roadmap, as does the site's
`site/data/registry/roadmap.json`. Both come from one function, `roadmap::infer`.

| horizon | the milestones in it |
|---|---|
| `now` | `ACTIVE` or `VERIFY`: work has started under it |
| `next` | `PLANNED`: nothing it requires is unfinished, so it can begin |
| `later` | `BLOCKED`: a milestone it requires is not `DONE` |
| `done` | `DONE` |

Inside a horizon the order is rank, then priority, then how many milestones each one unblocks.

What a release shipped comes from two committed facts. One is an issue's `completed_at` falling
between the previous release and this one. The other is a commit in the release's range naming
the issue, which the changelog already resolves. Neither moves when an unrelated commit lands.

What is inferred is labelled as inference:

- **pace**: a milestone's issues completed over the last five published releases.
- **forecast**: what remains divided by that pace. It is absent when nothing was completed,
  because no pace was measured.
- **signal**: two committed facts that disagree.

| signal | means |
|---|---|
| `shipped_unrecorded` | a released commit names an issue whose record still reads not started |
| `awaits_evidence` | every issue is done and the milestone's own evidence is not recorded |
| `no_issues` | no issue names the milestone: the outcome is specified and the work is not |
| `idle` | work has started, and nothing was completed or named in the window |

A signal never changes a status. A record is transitioned by the worker, and a milestone is
accepted by a person.

The served capability includes the work since the last release. The site's copy leaves it out,
because a committed file holding it would go stale at the next commit. The site is rebuilt with
every change to the trunk.

## The gate

> Each step is gated by the previous one being real.

That sentence used to sit under a hand-written table. It is now an invariant:

**A milestone whose dependencies are not all `DONE` is `BLOCKED`, whatever its own issues
say.** Finishing every issue inside a milestone does not carry it past a prerequisite that
has not been reached.

Reaching `ACTIVE`, `VERIFY` or `DONE` past an unmet dependency is not a status to display.
It is a contradiction, reported as `milestone_premature`.

The gate runs in rank order, so a block cascades the whole way down a chain rather than
one step.

## Order comes from the graph

The roadmap sequence is each milestone's topological rank in the dependency graph. The
`order:` field only breaks ties *within* a rank; it cannot move a milestone past something
it requires.

There is no list of versions anywhere: not in the records, not in the generator, not in the
site, and not here. Adding a milestone file puts it in the roadmap.

## What the roadmap graph refuses

The same refusals the work graph has, one level up, each by name:

| finding | means |
|---|---|
| `milestone_self_dependency` | a milestone requires itself |
| `milestone_unknown_dependency` | a milestone requires one that does not exist |
| `milestone_duplicate_dependency` | the same requirement is declared twice |
| `milestone_cycle` | the requirements form a loop, so no order exists |
| `milestone_premature` | a milestone is being executed past an unmet dependency |
| `empty_milestone` | a milestone is reachable and nobody has planned work in it |

```
bash test/run.sh 48_roadmap_graph
```

## Claims

A milestone may name the claims it would move:

```yaml
claims:
  - runtime-adapters
```

Those are `docs/CLAIMS.yaml` ids. The relation is declared, never inferred from prose that
happens to match.

Linking a claim to a milestone does not promote it. A claim becomes `guaranteed` by the
existing route and only that route — an implementation, a behavioural test, and CI running
that test. Accepting a milestone is not evidence about a claim.

## A milestone that shipped before the model existed

A milestone with no issues is `PLANNED` — an outcome nobody is executing — unless it
carries its own `evidence`, in which case the outcome was reached outside this model and
is recorded as reached.

That is how work delivered before the model existed is represented without inventing
issues for it. It is the one route to `DONE` that does not pass through an issue, and it
still passes through evidence.

## Superseded, not deleted

A milestone that has been replaced sets `superseded_by:` and derives `SUPERSEDED`. Roadmap
history is kept rather than rewritten; an accepted milestone is never edited away.

## Adding a milestone

Write one file:

```
.ai/repo/project/milestones/<slug>.yaml
```

It appears in `plan roadmap`, in `plan rgraph`, in the generated site data and in the
GitHub projection, because every one of those reads the directory. No route list, no
navigation entry, no version array and no generator branch names any milestone.

`test/cases/48_roadmap_graph.sh` adds a milestone mid-test and asserts it appears without
any list being edited.

## See also

- [`PLANNING.md`](PLANNING.md) — the records, derived status, waves, evidence, projections
- [`DOGFOODING.md`](DOGFOODING.md) — why this repository is required to use it
