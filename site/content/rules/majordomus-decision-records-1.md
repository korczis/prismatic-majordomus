+++
title = "Decision records"
description = "Decision records"
weight = 13
[extra]
kind = "rule"
slug = "majordomus-decision-records-1"
identity = "majordomus.decision-records@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/decision-records.v1.md"
+++
{% raw %}

## Rationale

Every entry in decisions.md carries the task, the head and the reason, so a decision can be found by the worker who needs it.

## Required behaviour

Every recorded decision carries the task, the head and the reason, so the worker who needs it can find it.

## Failure behaviour

A violation is a `WARN` finding under the category `records`; the command continues and exits as it otherwise would. Under `watch` it is reported as drift and the command exits 11.

## Verification

`mj_validate_decisions` decides it, dispatched from `check, doctor, watch`. The behavioural case `test/cases/21_decision_question.sh` proves it, and CI runs that case.
{% endraw %}
