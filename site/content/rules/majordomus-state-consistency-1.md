+++
title = "State consistency"
description = "State consistency"
weight = 51
[extra]
kind = "rule"
slug = "majordomus-state-consistency-1"
identity = "majordomus.state-consistency@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/state-consistency.v1.md"
+++
{% raw %}

## Rationale

The task record still describes this checkout — same branch, and HEAD at or ahead of the recorded commit.

## Required behaviour

A task record is trusted only while it still describes this checkout: same branch, HEAD at or ahead of the recorded commit.

## Failure behaviour

A violation is a `FAIL` finding under the category `state`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_state` decides it, dispatched from `check, finish, watch`. The behavioural case `test/cases/04_start_check.sh` proves it, and CI runs that case.
{% endraw %}
