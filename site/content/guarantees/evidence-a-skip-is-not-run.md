+++
title = "A test that declined to run reads not_run with the reason it gave, never proven and never failing, and a test that errored or timed out reads failing"
description = "A recorded skip is a test that declined to run: it measured nothing, so it proves nothing,"
weight = 182
[extra]
claim_id = "evidence-a-skip-is-not-run"
status = "guaranteed"
source = "docs/claims/evidence-a-skip-is-not-run.md"
+++
{% raw %}

## What it means

A recorded skip is a test that declined to run: it measured nothing, so it proves nothing,
and it failed nothing either. The claim it proves reads `not_run`, and its detail says the
test declined to run. A guaranteed claim resting on it is still a finding — a guarantee
nothing currently supports — and the finding says the test declined rather than that no run
was ever recorded.

A test that errored, or that the bound stopped before it finished, did not decline. It reads
`failing`, and the detail says which.

## How it works

`evidence::freshness` maps the recorded outcome before it compares anything: a skip is
`not_run` with the detail "the test declined to run"; a failure, a timeout and an error are
`failing` with "failed", "timed out" and "the harness could not run it". A result word the
recorder cannot classify is recorded as an error, so it reads `failing` and never a pass.

The finding's reason reads the judgement's detail, so a skipped guarantee is reported as
"the claim guarantees a behaviour whose test declined to run". The rules report uses the
same function, so a rule whose test was skipped reads `not run` rather than `failing`.

## How to see it

```bash
majordomus evidence show --state not_run --format json | jq '.claims[] | {id, detail}'
majordomus evidence show --findings
bash test/run.sh 502_evidence_is_judged_at_the_presented_revision
```

The case records a skip and asserts `not_run`, the detail and the finding's reason, then
records a result word nobody can classify and asserts `failing`.

## What it does not cover

The detail is the fixed sentence "the test declined to run". The reason the test itself gave
is not yet recorded in the ledger, so it cannot be shown here.

A skip is not a pass. A guarantee whose only recorded run is a skip stays a finding; this
claim changes what the finding says, not whether there is one.

## Why it exists

Every outcome other than a pass used to read `failing`, so a test that declined to run on a
machine without its precondition was reported as a broken behaviour. That is a false
finding, and a false finding teaches a reader to ignore findings. The opposite mistake —
reading a skip as a pass — would be worse, so a skip is placed exactly where it belongs: an
absence of proof, named as such.
{% endraw %}
