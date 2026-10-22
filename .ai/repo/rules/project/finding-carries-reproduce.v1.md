---
id: project.finding-carries-reproduce
version: 1
kind: rule
title: Every finding carries a reproduce command
description: Every finding a command reports names the command that reproduces it.
statement: Every finding a command reports names the command that reproduces it.
status: active
class: blocking
depends_on: []
tags: [findings]

x-majordomus:
  tests: [scripts/ci/finding-reproduce-check]
---

# Rationale

A finding without a way to see it again is an opinion; the reproduce command is what lets the next person, or the next session, check the fact rather than trust the report.

# Required behaviour

Every finding a command reports names the command that reproduces it.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

`scripts/ci/finding-reproduce-check` reads every call to a reporting helper in the shell
tool and the scripts — `mj_fail`, `mj_drift`, `mj_warn`, `mj_doctrine_fail` — and counts its
arguments, so a call that omits the reproduce command is a finding rather than a finding
that reads exactly like one that could not have it. A forwarding call is measured where it
is written; a call the gate cannot parse is reported as unreadable and counted, never
silently passed.
