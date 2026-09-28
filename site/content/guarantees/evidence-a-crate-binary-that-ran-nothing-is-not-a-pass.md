+++
title = "A crate test binary that ran no test or only a filtered subset is recorded as a skip, one with a failed test as a failure and one that printed no result as an error, whatever colour codes its output carries, and what no claim can name yet is listed as dropped"
description = "majordomus evidence record --crate-output <file> reads what cargo test printed and"
weight = 195
[extra]
claim_id = "evidence-a-crate-binary-that-ran-nothing-is-not-a-pass"
status = "guaranteed"
source = "docs/claims/evidence-a-crate-binary-that-ran-nothing-is-not-a-pass.md"
+++
{% raw %}

## What it means

`majordomus evidence record --crate-output <file>` reads what `cargo test` printed and
records one execution per integration test binary under `tests/`. What it records is what
that binary's run stated, and nothing more:

- a binary whose result line counts a failed test is `fail`, whatever word the line starts
  with;
- a binary that passed nothing, because every test was ignored or there was none, is
  `skip`;
- a binary that a name filter narrowed to a subset is `skip`, however many of that subset
  passed, because it is not the binary's proof;
- a binary whose result line carries no count, or that printed no result line at all before
  the next binary, is `error`: the harness could not say what ran;
- only a binary that ran its whole set and failed none is `pass`.

A skip reads `not_run` with the detail that the test declined to run, and an error reads
`failing` with the detail that the harness could not run it, so neither can make a claim
`proven`.

What the output held that no claim can name yet is not ignored. The crate's own unit-test
binary, its doctests, a binary outside `tests/` and a result line that no `Running` line
named are each listed in the recording's `dropped` list, with the report they came from and
the reason, and none of them is credited to the binary beside it.

## How it works

`evidence::read_crate_output` in `apps/majordomus-cli/src/evidence/record.rs` strips
terminal escape sequences from every line first, so a coloured run and a plain one are the
same text. A `Running tests/<name>.rs` line opens a binary; a `Running unittests <path>`
line, a `Doc-tests <crate>` line or a `Running` line naming any other path opens an entry
that will be listed rather than recorded. The next `test result:` line closes whatever is
open.

The result line is read by its counts: passed, failed, ignored and filtered out, each
optional. The first rule that holds decides: a failed count or the word `FAILED` is a
failure; the word `ok` with no passed count is an error; `ok` with nothing passed, or with
anything filtered out, is a skip; `ok` otherwise is a pass; any other word is an error. A
binary still open when the next binary, the doctests or the end of the output arrives got
no result line, and is an error.

The dropped entries are `EvidenceDropped` values (`producer`, `what`, `reason`) with the
producer `crate`. `evidence record` returns them as `dropped` in its JSON output and prints
one `dropped` line each in its text output. A recording of a suite report alone lists
nothing, and its output is unchanged.

## How to see it

```bash
cargo test --manifest-path apps/majordomus-cli/Cargo.toml 2>&1 | tee crate.log
majordomus evidence record --crate-output crate.log
majordomus evidence record --crate-output crate.log --format json | jq '.dropped'
bash test/run.sh 506_a_crate_binary_that_ran_nothing_is_not_a_pass
```

The case writes a coloured `cargo test` output with a unit-test binary, integration binaries
that passed, ran nothing, ran a filtered subset, failed, and printed no result, and the
doctests. It asserts each binary's recorded outcome, the dropped list in both output formats,
and the verdict each outcome reaches.

## What it does not cover

It reads one result per binary. Which tests inside a binary passed or failed is not
recorded, so a claim still names a whole binary; per-test results are later work.

A binary whose every test is compiled out on the machine that ran it reads as a skip. That
is the truth about that machine, not a defect: the run there proved nothing about it.

The unit tests and doctests that ran are listed, not recorded. No claim can name one of
them until a runner records them one by one.

## Why it exists

The recorder read a result line that started with `ok` as a pass. A binary whose tests were
all ignored, or a run filtered down to one test, printed `ok` and was recorded as the
binary's proof, so a claim could read `proven` on the strength of a run that measured
nothing. Reading the counts closes that, and listing what could not be recorded keeps a run
from looking smaller than it was. ADR 0087 (proposed), decision 6, states the rule: a report
states only what ran.
{% endraw %}
