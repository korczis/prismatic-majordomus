+++
title = "Questions store integrity"
description = "Questions store integrity"
weight = 41
[extra]
kind = "rule"
slug = "majordomus-questions-store-integrity-1"
identity = "majordomus.questions-store-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/questions-store-integrity.v1.md"
+++
{% raw %}

## Rationale

Every entry in open-questions.md parses, because a gate that cannot read an entry can be bypassed by mistyping one.

## Required behaviour

Every entry in the questions store parses, because a gate that cannot read an entry can be bypassed by mistyping one.

## Failure behaviour

A violation is a `FAIL` finding under the category `records`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_questions_store` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/21_decision_question.sh` proves it, and CI runs that case.
{% endraw %}
