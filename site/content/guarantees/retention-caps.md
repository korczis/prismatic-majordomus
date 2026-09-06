+++
title = "The append-only ledger and handover store have retention caps that are checked"
description = "state/ledger.jsonl and state/handovers/ only ever grow. The policy sets a cap for each (ledger.retention_max_lines, handover.retention_max_files); doctor and watch report when a cap is exceeded. Nothing is deleted by Majordomus; the cap makes growth visible before it becomes a problem."
weight = 35
[extra]
claim_id = "retention-caps"
status = "guaranteed"
source = "docs/claims/retention-caps.md"
+++
{% raw %}

## What it means

`state/ledger.jsonl` and `state/handovers/` only ever grow. The policy sets a cap for each (`ledger.retention_max_lines`, `handover.retention_max_files`); `doctor` and `watch` report when a cap is exceeded. Nothing is deleted by Majordomus; the cap makes growth visible before it becomes a problem.

## How it works

`lib/doctor.sh` counts ledger lines and handover files and compares each with the policy value, failing when over. `lib/watch.sh` reports the same condition as retention drift. The counts are printed even when under the cap, so the trend is visible on every run.

## How to see it

```bash
sed -i.bak 's/retention_max_lines: 5000/retention_max_lines: 1/' .ai/repo/policy.yaml
majordomus doctor
# FAIL retention   ledger — 3 lines over cap 1
```

## What it does not cover

Rotation and archiving are the operator's decision; Majordomus never removes a record. It tells you when to look.

## Why it exists

A session-notes directory in the source environment grew to ten gigabytes and over a thousand files, and a metrics store accumulated hundreds of forty-megabyte snapshots that nothing ever read back. Nothing had ever deleted anything because nothing had ever said the store was full.
{% endraw %}
