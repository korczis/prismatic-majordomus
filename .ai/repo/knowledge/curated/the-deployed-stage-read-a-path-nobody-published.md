---
schema: knowledge/v1
id: the-deployed-stage-read-a-path-nobody-published
kind: knowledge
class: lesson
title: The deployed stage read a path no deployment had ever published, and every check was green
description: The measurement of 2026-10-28 - why plan delivery proved deployed for no issue while majordomus.dev served the trunk, the two causes, and what now refuses it.
status: verified
epistemics: observed
date: 2026-10-28
tags:
  - post-mortem
  - delivery
  - pages
  - plan
provenance:
  origin: authored
  derived_from:
    - file:apps/majordomus-cli/src/plan/delivery.rs
    - file:apps/majordomus-cli/tests/plan_delivery.rs
    - file:.github/workflows/pages.yml
relations:
  - type: documents
    target: file:.ai/repo/project/issues/I4315.yaml
  - type: documents
    target: file:.ai/repo/adrs/0227-the-deployed-stage-reads-the-pointer-where-pages-published-i.md
---

# The deployed stage read a path no deployment had ever published

## What was observed

2026-10-28, origin/master 81ef931a3e1. The owner asked to see delivered results: issues,
milestones, intents. Three readers disagreed about one site:

| reader | answer |
|---|---|
| `majordomus served observe --commit origin/master` | the deployment serves 81ef931a3e15 |
| `majordomus delivery report` | 29 of 29 features on master, deployed, publicly verified |
| `majordomus plan delivery` | none 761, branch 94, pr_open 17, merged 8, released 155, **deployed 0** |

After `majordomus served forge` (50 deployments, 854 merged pull requests recorded):
none 722, branch 78, pr_open 14, merged 13, released 208, **deployed 0**, with
`publication unread: 661e09a85e7:site/static/releases/latest.json cannot be read`.

## The two causes

1. **The path.** `delivery::published_live` read `<deployed sha>:site/static/releases/latest.json`.
   Every Pages deployment since 2026-09-05 is a commit `scripts/site-deploy` pushes to
   `gh-pages`, carrying the built site, where the pointer is `releases/latest.json`
   (`git show 661e09a85e7:releases/latest.json` names 0.30.2). The reader was written on
   2026-10-16 (ee02079b1bc, I2740) and never once read a pointer.
2. **The observation.** Since 3e8225a4329 (2026-10-13) deployments are observed by
   `served forge` alone. The Pages job runs it; no executor, session or local server does, so
   a checkout first reads "no deployment observed", which hides cause 1 behind it.

## Why nothing was red

- The integration test committed the pointer at the authored path on the trunk and observed
  that commit as the deployment: it proved the reader against the reader's own assumption.
- The `ui-snapshot` job observed the forge and published the unread answer as an answer.
- `unread` is designed to be honest, not loud: an unread source keeps an issue unsettled
  and names the source, and nothing judged a source that could have been read.
- `served observe` writes a different record (`.ai/local/state/served/observations.jsonl`)
  that the plan does not read; running it looks like a fix and changes nothing.

## What refuses it now (I4315, ADR 0227)

- The reader derives the served path from the static root and tries it first, then the
  authored path; a commit with neither names both.
- `plan_delivery.rs` publishes a gh-pages commit outside the trunk's history, as the deploy
  does, and tests the source layout and the empty commit beside it.
- The `ui-snapshot` job fails when the publication is unread after `served forge` for any
  reason but "no successful deployment".

## The lesson

A fixture that writes the input the reader expects proves the reader, not the system. When a
reader consumes what another program produces, the fixture is produced the way that program
produces it, and the job that publishes the reader's answer is the one that refuses an
answer it could not have read.
