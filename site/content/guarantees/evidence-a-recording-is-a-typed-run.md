+++
title = "Every recording yields a run record naming its commit, the tree each report's run measured, its totals and what was absent, dropped or unknown, and a coverage summary joins it only bound to the commit and tree it was measured on"
description = "majordomus evidence record returns, beside what it merged into the ledger, the recording"
weight = 229
[extra]
claim_id = "evidence-a-recording-is-a-typed-run"
status = "guaranteed"
source = "docs/claims/evidence-a-recording-is-a-typed-run.md"
+++
{% raw %}

## What it means

`majordomus evidence record` returns, beside what it merged into the ledger, the recording
as one typed run, an `EvidenceRunRecord`:

- its id: the CI run it was (`ci:<provider>:<run>:<attempt>`), or the local commit and time;
- its origin, its commit and the weakest tree over the reports it was given;
- the measurement of each report;
- totals by outcome and by runner;
- which reports were absent and why, what the reports held that no claim can name
  (dropped), and the tests they named that this repository does not have (unknown);
- the executions, each of which maps back to the record's id.

`--run-record <file>` writes the same record. `--coverage <summary>` joins the summary
`scripts/rust-coverage --summary-json` wrote as an `EvidenceCoverage`, which cannot be built
without a full commit, carries the tree its run measured and the floors the threshold files
state, and is current only for its own commit on clean trees.

## How it works

`evidence::record` in `apps/majordomus-cli/src/evidence/record.rs` builds the record from the
same values it merged: the totals through `ledger::outcome_counts`, the one count the ledger's
own summary uses, and the dropped list the crate adapter produced, so the recording's answer
and the record never disagree. The types are in `apps/majordomus-cli/src/evidence/run.rs`
and `apps/majordomus-cli/src/evidence/coverage.rs`; `CommitId` refuses an abbreviated or
upper-case id when it is parsed and when it is deserialised.

In CI, `scripts/ci/evidence-collect` reads its manifest's totals, tree and absences from the
run record rather than computing them a second time from the ledger's rows.

## How to see it

```bash
majordomus evidence stamp --producer suite --report suite.tsv --out suite.provenance.json
majordomus evidence record --suite suite.tsv --provenance suite.provenance.json \
  --run-record run.json --format json | jq '.run_record | del(.executions)'
bash test/run.sh 505_a_recording_carries_what_its_run_measured
bash test/run.sh 506_a_crate_binary_that_ran_nothing_is_not_a_pass
```

## What it does not cover

There is no index of run records and no bound on how many are kept yet, and a run record
holds one result per test binary, not per test. Nothing reads a run record to judge a claim:
a later surface lets one withhold `proven` through the one monotone rule, and none does yet.

## Why it exists

What a recording knew as a run was thrown away: the ledger keeps one execution per test, and
a coverage summary carried no commit, so a number about some tree nobody could name. A typed
run keeps what the run measured together and binds coverage to the commit it describes. ADR
0087 (proposed) records the model.
{% endraw %}
