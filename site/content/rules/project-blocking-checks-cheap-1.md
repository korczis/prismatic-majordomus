+++
title = "Blocking checks are deterministic and cheap"
description = "Blocking checks are deterministic and cheap"
weight = 62
[extra]
kind = "rule"
slug = "project-blocking-checks-cheap-1"
identity = "project.blocking-checks-cheap@1"
status = "active"
source = ".ai/repo/rules/project/blocking-checks-cheap.v1.md"
+++
{% raw %}

## Rationale

A slow or flaky gate is worked around, and a gate that blocks work in progress trains people to skip it. Only what is settled and decidable may refuse.

## Required behaviour

A check that can stop a command is deterministic and cheap; work in progress is reported, never blocked.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. Every doctrine's class is declared in the rule package; test/cases/17_doctrine_enforcement.sh proves the class decides whether a command stops.
{% endraw %}
