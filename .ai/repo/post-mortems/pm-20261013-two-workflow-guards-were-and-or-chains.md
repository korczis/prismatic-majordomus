---
schema: post-mortem/v1
id: pm-20261013-two-workflow-guards-were-and-or-chains
kind: post-mortem
title: Two steps of the projection workflow used A && B || C and shell-lint refused them
description: SC2015 from shell-lint on a workflow file means a guard written as an and-or chain; write an if-statement.
status: resolved
date: 2026-10-13
runs:
  - workflow: validate
    id: "38364727993"
    commit: ba3fd916b3
    jobs: ["structure"]
    step: "the structure gates of the plan"
cause:
  class: defect
  summary: Two run blocks of .github/workflows/github.yml guarded a command with `A && B || C`, which shellcheck reports as SC2015 and the shell-lint gate refuses.
components:
  - file:.github/workflows/github.yml
  - file:scripts/ci/shell-lint
signals:
  - id: gate
    text: "shell-lint: FAILED"
  - id: code
    text: "SC2015"
  - id: plan
    text: "failed: shell-lint"
fixed_by: ["commit:8d852d79f0", "pr:1079"]
prevention: ["file:scripts/ci/shell-lint"]
tags: [structure, workflow, shell]
related: ["issue:I2650"]
---

# Two steps of the projection workflow used A && B || C and shell-lint refused them

## What failed

`structure` ended with `run-plan: 53 gate(s) ran in 942s; failed: shell-lint`. The gate
printed, for lines 122 and 146 of `.github/workflows/github.yml`, `shellcheck reported issue
in this script: SC2015:info:2:30: Note that A && B || C is not if-then-else. C may run when
A is true`.

## Root cause

Both steps wrote a guard as an and-or chain. `C` runs when `B` fails as well as when `A`
does, which is the defect shellcheck names. The session that fixed it reported that a local
run of the gate with another shellcheck version is silent on these lines. Which landing
brought the two steps, and whether the structure job had run on its merged tree, was not
read for this record.

## Fix

`8d852d79f0` (pull request 1079) rewrites both guards as if-statements.

## Prevention

The gate is the guard. That it names the shellcheck version it needs, so that a local run
either agrees with the runner or says it cannot, is issue I2639.
