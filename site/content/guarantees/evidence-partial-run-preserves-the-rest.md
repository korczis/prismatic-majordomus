+++
title = "Recording one test's result never erases the evidence recorded for any other test"
description = "The ledger is merged per test, not rewritten per run. Recording replaces the entry for"
weight = 167
[extra]
claim_id = "evidence-partial-run-preserves-the-rest"
status = "guaranteed"
source = "docs/claims/evidence-partial-run-preserves-the-rest.md"
+++
{% raw %}

## What it means

The ledger is merged per test, not rewritten per run. Recording replaces the entry for
every test the report names and leaves every other entry exactly as it was, so `bash
test/run.sh 84_distribution_model` followed by a recording updates one row and changes
nothing else about what the repository has proven.

This is what makes recording a single case worth doing at all. A whole-ledger write per
run would silently delete the evidence for every test the run did not include, turning a
one-case run into a repository that has proven one thing — which is worse than one that has
proven nothing, because it looks deliberate and nothing in the file says otherwise.

## How it works

`Ledger::merge` in `apps/majordomus-cli/src/evidence/ledger.rs` walks the executions it is
given and, for each, replaces the entry with the same test identity or appends a new one.
There is no path that truncates. The recorder in `record.rs` produces executions only for
the results the report actually named; a test that appears in no report is not written, and
the join will say `not_run` or keep reporting the older execution, whichever is true.

Three properties of the file follow from the same decision. Entries are sorted by test
identity and written with a two-space indent and a trailing newline, so two runs of the
same set produce the same bytes and a diff shows what changed rather than what moved. Only
the latest execution of each test is kept, because the question the ledger answers — *is
this claim proven now* — reads only the newest, and the durable record of what happened
over time is git, over this file. And a missing ledger and an empty one are the same
answer, `not_run` for every claim, neither of them an error: a fresh clone has the first
and a repository mid-adoption has the second.

A ledger whose declared version this executable does not read is refused with both numbers
rather than reinterpreted, for the same reason the merge is partial: a misread ledger is
worse than an absent one, because an absent one reports `not_run` and a misread one could
report a pass.

## How to see it

```bash
MJ_TEST_REPORT=all.tsv bash test/run.sh
majordomus evidence record --suite all.tsv --origin local
majordomus evidence show --format json | jq '.ledger.executions'

MJ_TEST_REPORT=one.tsv bash test/run.sh 84_distribution_model
majordomus evidence record --suite one.tsv --origin local
majordomus evidence show --format json | jq '.ledger.executions'   # unchanged
git diff --stat .ai/repo/evidence/ledger.json                       # one entry moved
```

`test/cases/124_evidence.sh` drives the same sequence in a fixture repository and asserts
the count of executions after the partial run, that the test the run named carries the new
duration and origin, and that the test it did not name still carries the old ones.

## What it does not cover

Two results for the same test in one recording are not reconciled; the last one written
wins, because the recorder does not judge results and has nothing to prefer between them.

Nothing prunes. An entry for a test that has since been deleted from the repository stays
in the ledger until a later recording replaces it or somebody removes it in a commit; it is
reported honestly — `majordomus evidence proves` says the source is not present — but it is
not swept away, because a file this repository can only shrink by hand is one whose
shrinking is reviewable.

Merging preserves what was recorded, not what is true. An old entry for a test that still
exists keeps proving what it proved at the commit it names, and the join is what decides
whether that still stands.

A recording that is not committed is a local opinion. The merge happens in the working
tree; until the ledger is in a commit, no other checkout and no reader of the published
site has seen it.

## Why it exists

Running the whole suite takes long enough that nobody does it to check one change, and a
subsystem that only accepted whole-suite runs would be a subsystem nobody fed. The partial
run is therefore the normal case, and the invariant that makes it safe has to hold without
anyone thinking about it — which is why it is asserted twice: as a unit test over `merge`
itself, and behaviourally, through the executable, in the fixture repository.
{% endraw %}
