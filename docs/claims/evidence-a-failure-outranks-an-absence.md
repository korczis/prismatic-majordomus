# A rule with a failing test reads failing whatever its other tests say

## What it means

A rule that names several tests takes one state from all of them. When any of those tests
is failing, the rule is `failing`, and for a blocking rule that is a finding — even when
another test it names was never run or declined to run. Otherwise the rule's state is the
weakest of the tests that can carry proof.

## How it works

`evidence::freshness::aggregate` is the one aggregation. It takes each part's state and
whether the part can carry proof, and returns `failing` if any part is `failing`; otherwise
the weakest part that can carry proof, by the states' declared order; otherwise the weakest
part of all; and nothing for no parts. The rules report calls it with each test's rule state
and whether a runner or a gate drives the test, after a missing path has already made the
rule `dangling` and before a dispatched rule without a case is adjusted.

The first step exists because the declared order ranks `failing` above `not run`. A plain
weakest-of over a failing test and a skipped one — which now reads `not run` — would read
`not run`, which is not a finding, and the failure would be hidden behind an absence.

## How to see it

```bash
majordomus-cli rules show <rule-id> --format json | jq '.proof.state, [.proof.tests[].state]'
majordomus-cli rules report --findings
bash test/run.sh 502_evidence_is_judged_at_the_presented_revision
```

The case adds a blocking rule naming two cases, records one failure and one skip, and
asserts that the rule is `failing` and that the report lists it as a finding.

## What it does not cover

It decides how states combine, not what each test's state is; that is the freshness table.
A path nothing drives still counts only when it is all a rule names, as before.

The entry preflight recounts the rules corpus its own way until it is moved onto this
function and onto the corpus verdict.

## Why it exists

Making a skip read `not run` instead of `failing` fixed a false finding, and it opened a
hole: a blocking rule whose tests were one failure and one skip would have read `not run`
and lost its finding. Counter-evidence must never be outranked by the absence of evidence,
so the precedence is written once, where every aggregation of states reads it.
