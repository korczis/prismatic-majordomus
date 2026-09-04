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
---

# Rationale

A finding without a way to see it again is an opinion; the reproduce command is what lets the next person, or the next session, check the fact rather than trust the report.

# Required behaviour

Every finding a command reports names the command that reproduces it.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. The reproduce field is part of mj_finding in lib/common.sh; cases across test/cases/ assert the [reproduce: ...] suffix.
