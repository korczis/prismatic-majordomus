# An observed model's capability is never stated above its evidence, and routing with --observed chooses on that evidence, refuses a stale observation and says what the choice rests on

## What it means

Every capability word of an observed model carries how it is known: `inferred` (the runtime's
kind of model usually can), `advertised` (the runtime's own metadata says so) or `probed` (a
bounded request just succeeded). A probe that failed refutes the word whatever was advertised.
`models route --observed` adds observed models after every declared one, loaded first, and
holds them to the observation's age, the runtime's locality, each required word's presence and
refutation, and — with `--min-evidence` — its standing. The selected model's reason names its
age, load state and the evidence of each required word.

## How it works

`observed::standing` derives each word's standing from the claims and probes;
`models::route_observed` (`apps/majordomus-cli/src/models/mod.rs`) is a pure function of the
catalogue, the observation, its age and the need, with `disqualify_observed` applying the
observation's checks in the declared order. The unit tests prove refutation, the evidence
order, `min_evidence`, staleness, locality, the configured-before-advertised context window
and that report order does not change the decision; case 3211 proves a probe raising
`advertised` to `probed` through the real executable.

## How to see it

```
majordomus models route --observed --local-only --require embeddings --min-evidence probed
majordomus models route --observed --local-only --require text --max-age 60
bash test/run.sh 3211_a_local_runtime_is_observed
```

## What it does not cover

`probed` proves a request shape once, not reliable quality; task-validated evidence is a later
issue of the local-inference milestone. Declared catalogue models stand on the catalogue's own
provenance and are not held to `--min-evidence`. Without `--observed` routing is the catalogue
alone, exactly as before.

## Why it exists

A runtime's word about a model is a claim, and a router that treats it as proof sends work to
a model that cannot do it. Stating the evidence beside every word, and letting a caller demand
more, keeps the choice explainable and the claim honest.
