# The Cockpit shows every intent with its derived stage at /cockpit/intents, and one intent's criteria, evidence, serving issues and realising work at /cockpit/intents/<id>

## What it means

The intent model has a view in the Cockpit. `/cockpit/intents` lists every intent with its stage, the criteria with current evidence, the providers realising it and its findings. `/cockpit/intents/<id>` shows one intent: each criterion with its evidence state, linked to the test object it names and to the issues serving it, the command that reproduces it, and every unit of work realising it with the provenance of its link.

## How it works

`apps/majordomus-cli/src/cockpit/intents.rs` renders the `intent_realization` capabilities and decides nothing itself, so the pages cannot disagree with `majordomus intent realization` or `intent explain`. An intent the repository does not hold is a 404.

## How to see it

```bash
majordomus serve          # then open /cockpit/intents
```

## What it does not cover

The pages read; they change nothing. Writing an intent, a gap or a critique stays a file edit.

## Why it exists

A derivation that only a command line can show is one a reviewer does not look at. The page puts the criterion, its test and the work beside each other, one click from each.
