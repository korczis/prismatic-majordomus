---
schema: adr/v1
id: adr-0075
kind: adr
title: Which work realises an intent is a join over the records the lifecycle already keeps, and closed work does not outrank evidence
status: proposed
date: 2026-09-17
tags:
  - intent
  - session
  - handover
  - evidence
related:
  - rule:project.an-intent-outlives-its-sessions
  - rule:project.work-serves-a-declared-intent
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:.ai/repo/adrs/0073-an-issue-names-the-intent-criterion-it-serves-and-coverage-is-derived.md
  - file:apps/majordomus-cli/src/intent_realization.rs
  - file:apps/majordomus-cli/src/capability/builtin/intent_realization.rs
  - file:lib/session.sh
  - test:test/cases/388_an_intent_is_realised_across_providers_and_held_to_reality.sh
provenance:
  origin: authored
---

# 75. Which work realises an intent is a join over the records the lifecycle already keeps, and closed work does not outrank evidence

## Context

ADR 0070 made an intent a typed record whose stage is derived from the plan and whose criteria
are met only by evidence. ADR 0073 made an issue name the criterion it serves. Neither says who
is realising an intent: which task, in which episodes, under which providers, carried across
which handovers. That lineage matters most exactly when the worker changes — a Claude Code
window hands over and a Codex window resumes — and it is exactly then that anything held by the
worker is lost.

Every fact the lineage needs is already recorded by something that does not depend on a worker
remembering it. The ledger stamps each `task.started`, `task.checkpoint`, `task.handed_over`
and plan transition with its episode. A closed session record lists the issues its episode
moved and the paths it left changed. A peer claim names its scope. The episode's open file names
its provider — but that file is removed at close, so the provider of a finished episode was
recorded nowhere durable.

Separately, the intent stage `verifying` means every milestone is DONE and not every criterion
has current evidence. Nothing said so loudly. An intent satisfied yesterday whose behaviour broke
today falls back to `verifying` and reads, to a reviewer of the plan, as finished work.

## Decision

**Realization is a read-only join, stored nowhere.** `crate::intent_realization` reads the
ledger's tasks (each with every episode that worked on it, its provider, and its handovers), the
shared layer's closed session records whose episode no local task holds, and the peer board, and
joins each unit of work to intents through issue and milestone. Two capabilities project it:
`intent_realization.work` (`majordomus intent realization`, `GET /api/v1/intents/realization`,
`majordomus_intent_realization`) and `intent_realization.explain` (`majordomus intent explain`,
`GET /api/v1/intents/explain`, `majordomus_intent_explain`).

**Every link carries its provenance**, from one mapping of the fact it was read from:

| via            | provenance | fact                                                        |
|----------------|------------|-------------------------------------------------------------|
| `named_issue`  | `declared` | the claim or the task's own words cite the issue             |
| `moved_issue`  | `observed` | a plan transition in the work's episode, or a record's issues |
| `branch_issue` | `derived`  | the branch name carries the issue id                          |
| `scope_overlap`| `inferred` | only when none of the above exists: an open issue's scope overlaps |

One link per issue, the strongest. An inferred link is never shown as a declared one.

**The episode's start line names its provider.** `session.started` carries `provider` and
`provider_session` when a provider opened the episode, so a closed episode keeps its provider on
the append-only record.

**Closed work does not outrank evidence.** Each intent reports its unmet criteria with the issues
serving each and its drift: `closed_work_contradicted` when every milestone is DONE and a
criterion's recorded run is failing or stale, `closed_work_unproven` when it has none,
`criterion_closed_unmet` when every issue serving a criterion is DONE and the criterion is not
met. `majordomus intent realization` exits 10 on `closed_work_contradicted`, and the
`intent-realization` gate runs it. A satisfied intent whose reality regresses is caught by that
gate, naming the criterion.

## Alternatives rejected

**An `intent` field on the task, the session or the handover.** It would be a second record of
what the plan already relates, filled in by the worker the lineage exists to outlive; and it
would be wrong the first time an issue moved milestones.

**A handover that copies the intent's state into its body.** A handover is read by the next
worker, not by the next derivation: a copy is stale the moment evidence moves, and would make
the handover a second satisfaction record.

**Scope overlap as an ordinary link.** Broad scopes overlap most issues, and presenting those as
facts would make the lineage look complete when it is guessed. They are kept, marked `inferred`,
and used only when nothing stronger exists.

**Stamping satisfaction when the last milestone closes.** Refused by ADR 0070 for the same reason
it is refused here: the plan closing is an event, the intent being true is an observation.

## Consequences

- A session ending, a provider changing or a peer reconnecting moves no intent; the join
  answers the same lineage from the records that remain.
- The peer board is memory of one server: claims appear in the join served by that server and
  not in a standalone command-line read. The durable half of the lineage is the ledger.
- Codex and Gemini hooks are being wired by another branch; this decision needs only that they
  open episodes through `session start --provider`, which is the entry point case 388 drives.
- The Cockpit intent view, the context compiler's use of the unmet criteria, and GitHub milestone
  reconciliation read this join and are not decided here.

### Amendment, 2026-10-15: a record that did no work, or did it before any intent, is not orphan work

`majordomus intent realization` on master c0e3585dc8 read 89 units of work and counted 80 as
serving no intent. All 80 are closed session records started between 2026-09-06 and
2026-10-04. Read against their own front matter, 66 record no issue, no commit and no changed
path other than session records (65 of them are `Recovered episode` records, whose commits and
changed paths `majordomus recover` cannot prove after the fact), and the other 14 started
before 2026-09-15, the earliest `created_at` of any intent record. None of them did work after
an intent existed and left it unlinked. A proposal to "bind the orphans to intents" cannot be
carried out under this ADR: a link is admitted only from a recorded fact, none exists, and a
session record is append-only. The count was wrong, not the records.

**An unlinked unit is classified by the facts it records, in this order, and only the last
class is orphan work:**

| class            | the unit                                                                                     |
|------------------|----------------------------------------------------------------------------------------------|
| `empty`          | is a session record that lists no issue, no commit and no changed path outside `.ai/repo/sessions/`, and whose own words cite no issue |
| `before_intents` | is not live, is not `empty`, and its earliest episode started before the earliest `created_at` of any intent record (every non-live unit, when no intent is declared) |
| `orphan`         | is anything else unlinked: work that was done, or is being done, while an intent existed, with no fact linking it to one |

A live unit (an active task, an attached claim) is never `empty` or `before_intents`: it is work
happening now, and it keeps the `work_serves_no_intent` warning exactly as before. No class is a
link: classifying a unit records no intent for it, and nothing writes to a session record.
`orphans` keeps its name in the JSON answer and now counts only the `orphan` class; the two new
counts sit beside it, so a reader that summed the old field reads a smaller, true number rather
than a missing one.

Rejected: binding the 80 records to intents (it fabricates `declared` or `inferred` links that
no fact supports and edits append-only records); deleting or archiving the empty records (they
are the recovery's evidence that an episode died, and retention is ADR 0091's, not this
report's); and leaving the count as it was (a report that calls 80 records orphan work when
none is sends people to bind work that does not exist, which is how the proposal arose).

The work is three slices, one pull request each:

1. **`realize()` classifies.** `IntentWorkUnit` gains `started_at` (session record
   `started_at`, a task's first `task.started`, a claim's `at`) and `commits` (a session
   record's `commits`); `crate::intent_realization` reads them where it already reads
   `changed_files` and `issues`. `IntentRealizedWork` gains `class` (`empty`,
   `before_intents`, `orphan`) on every unlinked unit, and `IntentRealization` gains `empty`
   and `before_intents` beside `orphans`. Unit tests in `intent_realization.rs` cover each class,
   the precedence (an empty record before the first intent is `empty`), a live unit that is never
   excused, and a repository with no intent. Proof: `realize_classifies_unlinked_work`.
2. **The CLI and the Cockpit show the three counts.** The text summary of
   `majordomus intent realization` reads `N serving no intent (E empty records, B before the
   first intent)` and each unlinked line names its class; the Cockpit intents page shows
   `orphans` as the statistic and the two others beside it. `tests/intent_realization.rs` and
   `commands/intent.rs` assert the new fields.
3. **A case holds it end to end.** A new case (number taken on the board when the slice starts)
   builds a fixture with one empty recovered record, one record with commits before the first
   intent, one record with commits after it, and one active unlinked task, and asserts
   `orphans` 2, `empty` 1, `before_intents` 1, and the warning on the task alone.

On c0e3585dc8 the classes read `empty` 66, `before_intents` 14, `orphan` 0.
