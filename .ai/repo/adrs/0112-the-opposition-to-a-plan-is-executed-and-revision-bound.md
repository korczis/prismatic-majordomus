---
schema: adr/v1
id: adr-0112
kind: adr
title: The opposition to a plan is executed by the tool, and a review authorises only the plan it reviewed
status: proposed
date: 2026-10-07
tags:
  - intent
  - planning
  - critique
  - governance
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0073-an-issue-names-the-intent-criterion-it-serves-and-coverage-is-derived.md
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:.ai/repo/adrs/0111-a-task-names-what-it-serves-and-the-binding-is-derived.md
  - rule:project.work-serves-a-declared-intent
  - rule:project.derived-once
  - file:apps/majordomus-cli/src/intent_opposition.rs
  - file:apps/majordomus-cli/src/capability/builtin/intent_opposition.rs
  - file:.ai/repo/project/intents/intent-opposition.yaml
  - test:test/cases/963_an_opposition_is_executed_and_names_the_plan_it_reviewed.sh
  - test:test/cases/964_a_review_of_another_plan_authorises_nothing.sh
---

# 112. The opposition to a plan is executed by the tool, and a review authorises only the plan it reviewed

## Context

ADR 0073 made the critique of a plan a typed record and refused execution when it was absent
or had a blocking finding open. That proves a file exists. Nothing ran the review; nothing
said which plan it reviewed, so a critique of last month's plan authorised today's; the
reviewer was a line of prose; and a record with `findings: []` that nobody ran anything over
read exactly like a review.

The owner asked that the opposition be executed, not merely represented by a required file,
that it be bound to the revision of the plan it reviewed, and that it need no model.

ADR 0073 also said, twice, that Majordomus does not reason about the gap or the plan, and
intent `intent-planning` carried that as an invariant ("it does not generate a plan or a
critique itself") beside another: no coverage, gap or critique status is stored.

## Decision

**The structural half of a review is derived, every time, and stored nowhere.** Everything
the coverage, the plan and the gap review report about one intent is that intent's structural
opposition. `intent_opposition::oppose` selects those findings and derives none: a failure
there is a blocking finding and a warning an advisory one. Because it is derived again at
every gate, a review cannot outlive a plan the checks would now reject.

**The recorded half is a reviewer's.** The one critique record per intent keeps what a
reviewing session found and how each finding was resolved. A finding may name its `source`
and a resolution its `resolved_by`; two classes are added, `invariant_conflict` and
`dependency_order`; a resolution planned into a cancelled issue is refused.

| what a review is asked to find | where it lands |
|---|---|
| a criterion with no work, weak work, or work with no purpose | structural, from coverage |
| a dependency that does not resolve, a cycle, premature execution, overlapping scope | structural, from the plan; `dependency_order` when a reviewer judges the order wrong |
| a gap condition left unanswered, an evidence reference that does not resolve | structural, from the gap review and the intent record |
| a challenged assumption | `unproven_assumption` |
| missing criteria or work | `missed_requirement`, `insufficient_work` |
| a conflict with an invariant | `invariant_conflict` |
| scope inflation | `unnecessary_work` |
| a risk, a missing surface, a missing verification | `regression_risk`, `surface_missing`, `delivery_verification` |

**One disposition, derived.** `reject` while a structural finding is blocking or a recorded
blocking finding is open; `accept_with_required_changes` when recorded blocking findings are
each planned into live work or rejected with a reason; `accept` otherwise. It is computed by
one function and written to no record, so the invariant that no critique status is stored
holds unchanged.

**A stamp says which plan was reviewed.** `reviewed_plan` hashes what a review judges: the
statement, invariants, non-goals, each criterion with its evidence kind and reference, each
live serving issue's own milestone, links, dependencies, scope and required evidence, and
the gap's conditions. It hashes no status, no wave, no title and no critique.
`majordomus-cli intent stamp <intent>` (capability `intent_opposition.record`) writes
`reviewed_revision`, `reviewed_at` and `reviewed_with` into the critique — three top-level
lines, by the line replacement `plan.transition` uses — and appends `opposition.recorded` to
the ledger. It writes no finding and no disposition, and creates a record with no findings
when the intent has none. The revision stamped is the one the executable derives at that
moment; no caller supplies it.

**A review of another plan authorises nothing.** The binding (ADR 0111) refuses work serving
an intent whose critique is stamped against a revision that is not the plan's
(`critique_stale`), whose plan a structural failure rejects (`plan_rejected`), and — under
policy `intent.opposition: required` — whose critique was never stamped
(`opposition_not_executed`). `intent validate` reports `critique_stale` and
`critique_not_stamped`: failures under `required`, warnings otherwise, so a critique written
before this decision turns no adopter's gate red.

**The brief is the answer.** `majordomus-cli intent oppose <intent>` (capability
`intent_opposition.review`) is the bounded input a reviewing session works from and the
verdict over what is found, on the command line, HTTP and MCP.

This supersedes two sentences of ADR 0073 — "Majordomus does not reason about the gap or the
plan" in its decision, and the same clause where it puts automatic derivation out of scope —
for the structural half only. Majordomus still authors no semantic finding and derives no
plan. Intent `intent-planning` is amended to say so.

## Consequences

- A stamp is evidence that the command ran and that the record's stamp lines were not edited
  carelessly afterwards. It is not proof against an author determined to forge one: the
  revision is a hash of public content. What makes the review executed is that the
  structural half is derived at every gate, whatever the record says.
- A reviewer may resolve a finding it raised itself. `resolved_by` makes that visible and no
  rule forbids it; who may accept a required change is a policy this decision does not make.
- No advisor is required and none is wired in. A consultation recorded by `majordomus
  reasoning` has no class and no subject, and lives in untracked checkout state; turning an
  advisor's answer into findings is done by whoever transcribes it into the critique. That
  mapping is not built.
- Earlier reviews of an intent are in git, and each run is in the ledger. There is no second
  append-only store of reviews.
- The writing capabilities are four: `plan.transition`, `reasoning.record`,
  `recover.orphans` and `intent_opposition.record`.
- Editing a criterion, an invariant, or a serving issue's links, dependencies, scope or
  required evidence makes the review stale. That is the cost of the stamp meaning something,
  and the reason the revision hashes nothing else.

## Alternatives rejected

- **Store the structural findings and the disposition in the critique.** They would be true
  of the plan they were computed over and stay in the record while the plan moved; it would
  also contradict the invariant that no critique status is stored.
- **A seal over the record's content.** With no secret it can be recomputed by anyone writing
  the file by hand, so it would distinguish nothing it claimed to.
- **A YAML emitter that rewrites the record.** The crate reads a restricted subset and has no
  writer for nested free text; a writer that had to round-trip through two readers to change
  three lines was not worth what it could corrupt.
- **Require an advisor for a review to count.** Correctness would depend on a provider being
  reachable, and CI admits none.
- **One policy key for binding and opposition.** They fail differently: binding is about
  whether a task says what it serves, opposition about whether the plan was reviewed as it
  stands, and a repository can need the second before it can afford the first.
