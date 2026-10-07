+++
title = "A recorded execution carries the commit and the tree that its run's own measurement stated, a measurement that names the report it belongs to and never hides a tracked change; a measurement of another commit or another report is refused, a local measurement that names no report is refused, a CI report without a measurement is refused, and a report recorded without one carries an unknown tree, which never reads proven"
description = "The recorder does not measure its own checkout. The run does, when it ends, with"
weight = 241
[extra]
claim_id = "evidence-tree-is-measured-by-the-run"
status = "guaranteed"
source = "docs/claims/evidence-tree-is-measured-by-the-run.md"
+++
{% raw %}

## What it means

The recorder does not measure its own checkout. The run does, when it ends, with
`majordomus evidence stamp`, and the recorder carries that measurement into the executions
of the report it names:

- the commit and the tree an execution is recorded with are the ones the run's stamp stated,
  whatever the recorder's own checkout looks like when it records;
- the tree ignores the evidence ledger and excludes only the run's own untracked outputs; a
  change to a tracked file is always a change;
- when the run's tree was clean, the test's digest is taken from the commit, the bytes the run
  executed;
- a local report recorded without a measurement carries the tree `unknown` and never reads
  `proven`.

The recorder refuses, before it writes anything, a measurement of another commit, a
measurement of other bytes than the report's, a measurement for a report that was not given
or keyed to another producer, a local or release measurement that names no report, and a CI
report without a measurement.

## How it works

`evidence::stamp` in `apps/majordomus-cli/src/evidence/provenance.rs` reads HEAD, checks each
`--exclude` (refused when it is a pattern, names the whole checkout, is not a repository path
or holds tracked files, which `git ls-files` answers), digests the report, and measures the
tree through `git::working_tree_excluding`, the one tree measurement, which hides an
excluded path only while it is untracked. The result is an `EvidenceProvenance`: the commit,
the tree, what was excluded, the time, the producer's toolchain, the recorder's version, the
report's path (repository-relative or its file name, never a machine path) and digest, the
host and the CI run.

`evidence record --provenance [<producer>=]<file>` resolves each file to the report it
measures, and `evidence::record` in `apps/majordomus-cli/src/evidence/record.rs` checks
every measurement against HEAD and against the report's bytes before it reads a report or
writes a ledger. Each execution takes its report's measured tree, or `unknown` without one,
and A1's freshness row reads a tree that is not `clean` as `inputs_unchanged`.

## How to see it

```bash
MJ_TEST_REPORT=suite.tsv bash test/run.sh
majordomus evidence stamp --producer suite --report suite.tsv --out suite.provenance.json
majordomus evidence record --suite suite.tsv --provenance suite.provenance.json
majordomus evidence claim evidence-tree-is-measured-by-the-run
bash test/run.sh 505_a_recording_carries_what_its_run_measured
```

The case stamps a fixture with and without its run's outputs excluded, refuses the excludes
that would hide a tracked edit, records a dirty stamp over a clean recorder and a clean stamp
over a dirty one, takes a clean run's digest from the commit, and asserts each refusal leaves
the ledger as it was.

## What it does not cover

A stamp measures the checkout when it is taken, after the report was written, and not the
tree while the run executed: a run on a dirty tree that is cleaned before the stamp reads
clean. A later slice measures at the start and at the end of a local suite run.

Until CI stamps with its report, a CI measurement is the producing job's tree file, which
names no report, so it is bound to the commit and not to the report's bytes.

A runner whose environment lies about its commit, and a person editing the ledger by hand,
are outside what a measurement can see.

The monotone rule that lets a run record withhold `proven` is not applied by any surface yet.

## Why it exists

The recorder stamped every execution with the tree it saw when it recorded, vouching for a
tree it did not see: CI records in a fresh checkout, so the tree it stamped was a constant
rather than a measurement, and a local run could be recorded after its checkout changed. ADR
0087 (proposed), decision 6, states the rule this carries out: the run measures, the
recorder carries, and a tree nobody measured is not clean.
{% endraw %}
