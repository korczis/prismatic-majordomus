+++
title = "A recording made with --ledger local writes only the ignored local ledger, changes no tracked file and moves no verdict"
description = "majordomus evidence record --ledger local merges its executions into"
weight = 232
[extra]
claim_id = "evidence-local-recording-stays-local"
status = "guaranteed"
source = "docs/claims/evidence-local-recording-stays-local.md"
+++
{% raw %}

## What it means

`majordomus evidence record --ledger local` merges its executions into
`.ai/local/evidence/ledger.json`, which the repository ignores, instead of the tracked ledger.
It changes no tracked file, so the checkout stays as it was, and it moves no verdict: every
claim reads what the tracked ledger says, exactly as before the recording. The default stays
the tracked ledger.

## How it works

`ledger::LedgerTarget` in `apps/majordomus-cli/src/evidence/ledger.rs` names the two ledgers,
and `Ledger::load_from` and `Ledger::save_to` read and write the one a recording names; a bad
local ledger is refused in its own name. Verdicts are derived from the tracked ledger only,
so nothing written to the local one reaches them.

## How to see it

```bash
majordomus evidence stamp --producer suite --report suite.tsv --out suite.provenance.json
majordomus evidence record --suite suite.tsv --provenance suite.provenance.json --ledger local
git status --porcelain
bash test/run.sh 505_a_recording_carries_what_its_run_measured
```

## What it does not cover

No surface reads the local ledger yet: a person's own runs are kept, and nothing shows them.

## Why it exists

A person who runs the suite locally to see where they stand should not have to change a
tracked file, and an unreviewed local run must not move a verdict the repository states. ADR
0087 (proposed), decision 1: a verdict is derived from the tracked ledger only.
{% endraw %}
