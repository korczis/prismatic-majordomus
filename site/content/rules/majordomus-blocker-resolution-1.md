+++
title = "Blocker resolution"
description = "Blocker resolution"
weight = 3
[extra]
kind = "rule"
slug = "majordomus-blocker-resolution-1"
identity = "majordomus.blocker-resolution@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/blocker-resolution.v1.md"
+++
{% raw %}

## Rationale

No task can be completed while any question on this branch is unresolved; it can still be finished as blocked, partial, no_match or failed.

## Required behaviour

No task is accepted as completed while any question on the branch is unresolved.

## Failure behaviour

A violation is a `FAIL` finding under the category `blockers`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_blockers` decides it, dispatched from `check, finish`. The behavioural case `test/cases/17_doctrine_enforcement.sh` proves it, and CI runs that case.
{% endraw %}
