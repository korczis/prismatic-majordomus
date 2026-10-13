+++
title = "Two steps of the projection workflow used A && B || C and shell-lint refused them"
description = "Two steps of the projection workflow used A && B || C and shell-lint refused them"
weight = 7
[extra]
kind = "post-mortem"
slug = "pm-20261013-two-workflow-guards-were-and-or-chains"
identity = "pm-20261013-two-workflow-guards-were-and-or-chains"
status = "resolved"
source = ".ai/repo/post-mortems/pm-20261013-two-workflow-guards-were-and-or-chains.md"
+++
{% raw %}

## Two steps of the projection workflow used A && B || C and shell-lint refused them

### What failed

`structure` ended with `run-plan: 53 gate(s) ran in 942s; failed: shell-lint`. The gate
printed, for lines 122 and 146 of `.github/workflows/github.yml`, `shellcheck reported issue
in this script: SC2015:info:2:30: Note that A && B || C is not if-then-else. C may run when
A is true`.

### Root cause

Both steps wrote a guard as an and-or chain. `C` runs when `B` fails as well as when `A`
does, which is the defect shellcheck names. The session that fixed it reported that a local
run of the gate with another shellcheck version is silent on these lines. Which landing
brought the two steps, and whether the structure job had run on its merged tree, was not
read for this record.

### Fix

`8d852d79f0` (pull request 1079) rewrites both guards as if-statements.

### Prevention

The gate is the guard. That it names the shellcheck version it needs, so that a local run
either agrees with the runner or says it cannot, is issue I2639.
{% endraw %}
