+++
title = "A profile declares which context a worker should load and no more"
description = "Each profile carries context toggles — task, current state, decisions, relevant files, failing output, recent history depth, architecture notes — and the always-loaded instruction file tells the worker not to read the whole repository to orient but to load what the profile names. The routine profile loads the task and the current state; deep-work adds architecture notes and two hundred commits of history."
weight = 52
[extra]
claim_id = "minimum-context"
status = "advisory"
source = "docs/claims/minimum-context.md"
+++
{% raw %}

## What it means

Each profile carries context toggles — task, current state, decisions, relevant files, failing output, recent history depth, architecture notes — and the always-loaded instruction file tells the worker not to read the whole repository to orient but to load what the profile names. The `routine` profile loads the task and the current state; `deep-work` adds architecture notes and two hundred commits of history.

## How it works

The toggles are validated fields in the profile; `check --explain` prints them for the active task; the projected body carries the rule "Load minimum sufficient context" as the second of ten. The always-loaded budget (a guaranteed claim) caps the one file every session must read.

## How to see it

```bash
majordomus check --explain | grep '^  context\.'
```

## What it does not cover

**Advisory.** Nothing measures what the worker actually read. The budget on the always-loaded file is the only guaranteed part of context control in v0.1; runtime clamps on read size are planned.

## Why it exists

Context per session was the largest cost term in the environments studied, and the least managed: a session ran out of context mid-mission, a best-practices document prescribed re-sending the objective every fifty messages, and a one-megabyte knowledge store was referenced from nowhere.
{% endraw %}
