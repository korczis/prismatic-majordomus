---
schema: post-mortem/v1
id: pm-20261013-the-handbook-failed-three-structure-gates
kind: post-mortem
title: The handbook sources failed the design, ui-check and lease-reader gates on the trunk
description: Sources imported from outside the repository are judged by every structure gate; run the structure job on them before they land.
status: resolved
date: 2026-10-13
runs:
  - workflow: validate
    id: "38328889515"
    commit: 56625981f8
    jobs: ["structure", "suite-shard (2)"]
    step: "the structure gates of the plan"
cause:
  class: defect
  summary: The handbook's Czech edition landed with colours chosen outside share/design/tokens.yaml and with text the ui-check and lease-reader gates refuse.
components:
  - file:handbook/build.py
  - file:scripts/ci/design-check
  - file:scripts/ui
  - file:scripts/ci/lease-reader-check
  - file:share/design/tokens.yaml
signals:
  - id: plan
    text: "failed: ui-check lease-reader design"
  - id: case
    text: "FAIL 110_lease_reader"
  - id: design
    text: "a colour chosen outside share/design/tokens.yaml"
fixed_by: ["pr:1062"]
prevention: ["file:scripts/ci/design-check", "file:scripts/ci/lease-reader-check", "test:test/cases/110_lease_reader.sh"]
tags: [structure, handbook, design]
related: ["issue:I2650"]
---

# The handbook sources failed the design, ui-check and lease-reader gates on the trunk

## What failed

`structure` ended with `run-plan: 53 gate(s) ran in 972s; failed: ui-check lease-reader
design`, and case `110_lease_reader` failed in shard 2. The session that read the job log
that night counted ninety findings of `a colour chosen outside share/design/tokens.yaml`,
all under `handbook/cs/`.

## Root cause

The handbook's sources were brought into the repository as they had been written outside
it (ADR 0149): inline colours of their own, and scripts and prose the two other gates read
as a second user interface and a second reader of the lease. Whether the landing that
brought them had run the structure job on its merged tree was not established for this
record.

## Fix

Pull request 1062 (`61d4417675`) makes the handbook wear the declared design and clears the
three gates.

## Prevention

The three gates and case 110 are the guard. The structure job on the merged tree before a
pinned merge is the landing rule since that day.
