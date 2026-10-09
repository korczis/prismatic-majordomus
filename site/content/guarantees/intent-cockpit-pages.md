+++
title = "The Cockpit shows every intent with its derived stage and verdict at /cockpit/intents, and one intent's verdict with what holds it back, its criteria, guards, evidence, the review of its plan, serving issues and realising work at /cockpit/intents/<id>"
description = "The intent model has a view in the Cockpit. /cockpit/intents lists every intent with its stage, its verdict, the required criteria with current evidence, the providers realising it and its findings. /cockpit/intents/<id> answers why one intent stands where it does without a command being run: the verdict with each required criterion holding it back and each violated guard, linked to its row; each criterion with whether it is optional, its evidence state and the run it was judged by, linked to the test object it names and to the issues serving it; each guard as violated, holding or not judged; the review of its plan with where the critique stands against the plan, the disposition, the stamp, every finding of both halves with its resolution, and the recorded gap; and every unit of work realising it with the provenance of its link."
weight = 116
[extra]
claim_id = "intent-cockpit-pages"
status = "guaranteed"
source = "docs/claims/intent-cockpit-pages.md"
+++
{% raw %}

## What it means

The intent model has a view in the Cockpit. `/cockpit/intents` lists every intent with its stage, its verdict, the required criteria with current evidence, the providers realising it and its findings. `/cockpit/intents/<id>` answers why one intent stands where it does without a command being run: the verdict with each required criterion holding it back and each violated guard, linked to its row; each criterion with whether it is optional, its evidence state and the run it was judged by, linked to the test object it names and to the issues serving it; each guard as violated, holding or not judged; the review of its plan with where the critique stands against the plan, the disposition, the stamp, every finding of both halves with its resolution, and the recorded gap; and every unit of work realising it with the provenance of its link.

## How it works

`apps/majordomus-cli/src/cockpit/intents.rs` renders `intent_realization.work`, `intent_realization.explain` and `intent_opposition.review` and decides nothing itself, so the pages cannot disagree with `majordomus intent realization`, `intent explain` or `intent oppose`. Each card is a function of the answer it renders. The page chooses only the colour a word is read in, and an accepting disposition is coloured only under a review that is current, so a plan nobody reviewed never reads as healthy. `apps/majordomus-cli/tests/cockpit_intents.rs` compares the words on the served page with the command line's answer on the same repository. An intent the repository does not hold is a 404.

## How to see it

```bash
majordomus serve          # then open /cockpit/intents
```

## What it does not cover

The pages read; they change nothing. Writing an intent, a gap or a critique stays a file edit, and stamping a review stays `majordomus intent stamp`. The page of one intent derives the intent model twice, once for each capability it asks. How an intent's plan is projected to GitHub is not shown. An invariant written as plain text is listed and judged by nothing; only a guard is judged.

## Why it exists

A derivation that only a command line can show is one a reviewer does not look at. The page puts the verdict, the criterion, its test, the review and the work beside each other, one click from each.
{% endraw %}
