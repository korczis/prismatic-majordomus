# Each unmet criterion of an intent carries what stands in its way and the action that justifies, each intent one outcome, and a gap observation the evidence now contradicts is named, all derived on every read with nothing written

## What it means

After work or evidence changes, the realization answers what still stands between an intent and its criteria: per unmet criterion one word — `progressing`, `blocked`, `needs_evidence`, `failed`, `exhausted` or `unknown` — with the row of ADR 0117's table that decided it and the next action, and per intent one outcome. A criterion a gap observed satisfied, served by no issue, whose evidence now fails, is warned `gap_observation_contradicted` by `intent validate`. Nothing is stored and nothing is performed.

## How it works

`apps/majordomus-cli/src/intent_remains.rs` reads the criterion's evidence state and its coverage entry (the live issues serving it and the strength), exactly as `intent validate` computes them, and fills them into the realization that `intent_realization.work` answers on the command line, HTTP and MCP. Each intent also carries `review_revision` (what a review is stamped with, ADR 0112) and `remains_digest`; a caller passing an earlier reading's two values is told `unchanged`, `remains_moved` or `plan_changed`. The warning is emitted where the gaps, the coverage and the evaluated criteria meet.

## How to see it

```bash
majordomus intent realization --intent <id>     # outcome, and remains, basis and next per unmet criterion
majordomus intent validate                      # gap_observation_contradicted, a warning
```

`test/cases/969_a_gap_observation_the_evidence_contradicts_is_named.sh` breaks a criterion a gap observed and finds the warning, the unchanged exit, the advisory finding of `intent oppose` and a byte-identical tree; `apps/majordomus-cli/tests/intent_remains.rs`, `intent_remains_surfaces.rs` and `intent_remains_since.rs` hold the table, the surfaces and the comparison of readings.

## What it does not cover

Who may act: issues carry no actor, so "needs authorisation" is not emitted. Whether a plan keeps failing or oscillates: no history of readings is kept. A pass that broke under open work reads `progressing` on `work_open_failing`, not told apart from a test written first. The gap is never rewritten; the remedy for a contradicted observation is an issue that serves the criterion.

## Why it exists

A reader assembled what still prevents an intent from four findings and two lists, and no read said what to do next; and coverage took a gap's satisfied observation as served, so a criterion that later broke was owed no work and nothing said so.
