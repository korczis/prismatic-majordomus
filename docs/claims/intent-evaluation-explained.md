# A criterion whose evidence was judged carries the run that decided it and what changed since, and a criterion declared optional holds neither the verdict nor the satisfied stage back

## What it means

Told that a criterion is stale, a reader should not have to re-derive why. The engine already knows which recorded run it judged and which inputs changed since; it now says so. And an intent can record something it would like to become true without letting it block the verdict forever.

## How it works

Each criterion whose test or claim has a recorded run carries an `evaluation`: the run's commit, working tree, outcome and time, the inputs of the evidence that changed since, and the evidence module's own sentence. It is derived with the state it explains and stored nowhere. "Considered" is one run, the latest, because that is the only run the judgement reads.

`optional: true` on a criterion means it is evaluated and shown like any other, and is counted in neither the verdict, nor `met`, nor the `satisfied` stage; `optional` on the intent carries how many there are. An optional criterion no work covers is the warning `optional_criterion_uncovered`, where a required one is a failure. An intent whose criteria are all optional is refused: `intent_without_required_criterion`.

## How to see it

```bash
majordomus-cli intent show <id>        # under each criterion: the run it was judged by, and what changed since
majordomus-cli intent explain <id>     # the same, as sentences
```

`apps/majordomus-cli/tests/intent_satisfaction.rs` records a passing run, changes the code under test and reads the changed path in the evaluation; and makes a criterion optional and reads a satisfied verdict over the required one alone.

## What it does not cover

A criterion settled by a `command` or a `deployment` has no recorded run and no evaluation: the ledger records runs of tests and claims. No earlier run is listed, only the latest. No rule name or policy version is given beyond the `proof` word the criterion already carries. A recorded gap must still answer an optional criterion.

## Why it exists

The evidence module computed the reasons and the intent engine kept one word. A derivation that cannot say why is one a reader has to take on trust, which is what the evidence ledger was built to make unnecessary.
