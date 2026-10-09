+++
title = "Each observation an episode recorded becomes, at that episode's end and with no model, a candidate of its own that says what it is about, and knowledge candidates groups the records about one subject with the episodes behind them; an episode that observed nothing records that it found nothing"
description = "When an episode ends, every observation it recorded with knowledge observe becomes a knowledge candidate — an observed lesson awaiting review — that says what it is about, without a model and without anyone asking. knowledge candidates groups the records about one subject, so the same defect seen by three sessions reads as one group with three episodes behind it. An episode that observed nothing writes no candidate and still records knowledge.derived with written: 0: it was evaluated and nothing was found."
weight = 234
[extra]
claim_id = "an-observation-becomes-a-grouped-candidate"
status = "guaranteed"
source = "docs/claims/an-observation-becomes-a-grouped-candidate.md"
+++
{% raw %}

## What it means

When an episode ends, every observation it recorded with `knowledge observe` becomes a knowledge candidate — an observed lesson awaiting review — that says what it is about, without a model and without anyone asking. `knowledge candidates` groups the records about one subject, so the same defect seen by three sessions reads as one group with three episodes behind it. An episode that observed nothing writes no candidate and still records `knowledge.derived` with `written: 0`: it was evaluated and nothing was found.

## How it works

The deriver's table gains one row (ADR 0118): `observation.recorded` yields a `lesson` with epistemics `observed`, `about` set to the subject and `route` derived from it. Its id is ADR 0091's — the episode followed by a digest of the kind, the subject and the statement — so every file is one episode's own, a second derivation over the same ledger writes nothing, and two branches that observed the same thing merge without a conflict. Evidence references that resolve now (a commit git knows, a tracked file or test) join `derived_from`; every reference is listed under `# Evidence`. The grouping is read, never written: the shell listing and `knowledge_base.candidates` group candidates and rejected records by `about`, listing each episode once.

## How to see it

```bash
majordomus session start
majordomus knowledge observe --kind defect --subject lib/a.sh "a.sh prints the wrong word"
majordomus session end                      # the boundary derives the candidate
majordomus knowledge candidates             # ... about lib/a.sh  route project  awaiting 1  rejected 0  episodes 1
majordomus knowledge candidates --json | jq '.groups'
```

## What it does not cover

It does not decide whether an observation is right, or act on it: a candidate is reviewed, promoted or rejected by a person or an authorised agent. Friction the ledger records on its own (`task.refused`, `task.gate`) is not derived; which of those lines are friction rather than the tool working is a later decision.

## Why it exists

`.ai/repo/adrs/0118-a-session-leaves-what-it-observed-and-one-candidate-per-defect-is-routed.md`: sessions should improve the tool by being used, not by being told to reflect, and a recurring defect must read as one thing without any episode writing another's file.
{% endraw %}
