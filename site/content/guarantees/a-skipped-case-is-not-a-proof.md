+++
title = "A test case that declines to run is reported as a skip and recorded as one, so the evidence ledger can never read a case that asserted nothing as the proof of the claim it names"
description = "A case in test/cases/ may be unable to run where it finds itself: no jq, no zola, no"
weight = 191
[extra]
claim_id = "a-skipped-case-is-not-a-proof"
status = "guaranteed"
source = "docs/claims/a-skipped-case-is-not-a-proof.md"
+++
{% raw %}

## What it means

A case in `test/cases/` may be unable to run where it finds itself: no `jq`, no `zola`, no
`node_modules`, no browser, no built executable. Sixty of them say so. Until this change
they said it the only way the runner understood — printing a line about skipping and
exiting 0 — and exit 0 is the status `test/run.sh` writes `ok` for.

So the report the runner hands `majordomus evidence record` carried the same word for two
different events: a case that ran every assertion it was written to make, and a case that
made none. The recorder is honest about what it is given and stamps both with the commit,
the digest and the time. The claim named by the case that declined then read as supported,
on the strength of a run that measured nothing, and the only surface that could have said
otherwise was the log nobody reads after a green.

A skip now has its own word from end to end: the case calls `skip`, which exits 4 and
leaves a mark the runner named, the runner prints `skip` and writes `SKIP`, and the ledger
records `skip` — an outcome that does not prove. It is still not a failure: the run's exit
status does not change, and a run whose cases all declined is a run that happened rather
than "no cases found". And a failure is not a skip: a case that ends with 4 without having
called `skip` is a failed case.

## How it works

`test/lib.sh` declares the status and the helper:

```bash
MJ_SKIP_STATUS=4
skip() {
  printf '    skip: %s\n' "$*"
  if [ -n "${MJ_SKIP_MARK:-}" ]; then printf '%s\n' "$*" > "$MJ_SKIP_MARK"; fi
  exit "$MJ_SKIP_STATUS"
}
```

`test/run.sh` knows the same number. `run_case` propagates the case's own status instead of
collapsing every non-zero to 1, and then maps it: 0 is a pass, 4 is a skip when the mark is
there, everything else is a failure. The runner's own statuses — 2 for a fixture it could
not set up, 3 for the bound firing — are returned by the runner itself and are never read
from the case, so a case that exits 3 of its own accord is a failed case and not a reported
timeout.

The status alone is not the declaration, because a case runs under `set -e` and ends with
the status of whatever command failed. `jq -e` fails with 4 when it produced no result —
which is what it does on the empty output of a command that broke — and the cases in
`test/cases/` assert with bare `jq -e` lines throughout. A runner that read 4 as a skip
would record each of those failures as a case that declined and keep the run green: the
defect this claim closes, arriving from the other side. So `run_case` names a file beside
the case's fixture in `MJ_SKIP_MARK`, `skip` writes it, and a 4 is a skip only when the
file exists.

`verdict` gains one arm. The tally line gains a third number, and the report's result field
gains `SKIP` beside `ok`, `FAIL` and `TIMEOUT`. The parallel phase renders its verdicts
from files its workers wrote, through the same function, so both phases write the same
word — a runner that told the truth serially and `ok` in parallel would be worse than one
that never told it, because CI runs the suite in parallel.

Nothing in the Rust model changed. `Outcome::Skip` was already there, `Outcome::parse`
already read `skip` and `skipped`, and `Outcome::proves()` was already false for it. What
was missing was a runner that could ever write the word: the model had a vocabulary the
producer could not speak.

The counter also feeds the runner's two "nothing ran" refusals. `bash test/run.sh
09_site_mobile_first` on a machine without `zola` selects a case that exists and declines;
before, that was `0 passed, 0 failed` and exit 2, "no case matches", which sends a reader
looking for a file that is there.

## How to see it

```bash
bash test/run.sh 413_a_skipped_case_is_not_a_proof
MJ_TEST_REPORT=suite.tsv bash test/run.sh 09_site_mobile_first && cat suite.tsv
majordomus evidence claim a-skipped-case-is-not-a-proof
```

`test/cases/413_a_skipped_case_is_not_a_proof.sh` drives the real runner over a harness of
three cases of its own — one that passes, one that declines, one that fails — and asserts
the three words in both phases. It adds a fourth that fails through `jq -e` with status 4
and never calls `skip`, first checks that it really ends with 4, and asserts that both
phases write `FAIL` for it and turn the run red while the declared skip beside it stays
`SKIP`. It then records `SKIP` for a guaranteed claim in a fixture
repository and asserts the claim is not proven and stays a finding, and records the
identical report with `ok` in that one field and asserts that it is proven: the difference
between the two is the word, which is what makes the first an assertion about the word
rather than about the fixture.

## What it does not cover

It does not make a passing case an exercised case. A case that runs to the end having
asserted nothing — because its assertions are inside a branch nothing took, or because it
was written that way — still exits 0 and is still recorded as a proof. This closes the
declared skip, which is the half a machine can see; the other half is review, held by the
rule `project.no-claim-without-test`.

It does not audit the reasons. A case that declines because `jq` is absent and a case that
declines because a browser could not start are one word here, and whether a skip should
have been a failure in that environment is a judgement the case itself makes — several
cases already fail rather than skip when `CI` is set, and that stays their business.

It does not reach the crate. `cargo test`'s output is parsed per test binary, where an
ignored test is invisible inside a binary that reports `ok`; the suite's TSV is the surface
this claim is about.

## Why it exists

This repository has hit this exact failure before, from the other end: a case whose
`node_modules` were absent turned red into green, and the memory of it is one line long — a
skipped case reports ok. The evidence fabric raises the price. Before it, a skip recorded
as `ok` cost a reader one misleading line in a log; now it is written into a tracked ledger
with a commit and a digest beside it, published, and read back by a gate as the proof of a
guarantee. A subsystem whose whole argument is evidence over claims cannot have its
producer say `ok` for "I did not look".
{% endraw %}
