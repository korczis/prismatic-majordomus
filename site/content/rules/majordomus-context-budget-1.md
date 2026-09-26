+++
title = "Context budget"
description = "Context budget"
weight = 10
[extra]
kind = "rule"
slug = "majordomus-context-budget-1"
identity = "majordomus.context-budget@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/context-budget.v1.md"
+++
{% raw %}

## Rationale

The always-loaded projection stays within its line budget, every reference in it resolves, it states no count that will go stale, and the assembled context fits the builder's own budget.

## Required behaviour

The always-loaded projection stays within its budget, every reference in it resolves, it states no count that will go stale, and the assembled context fits its own budget.

## Failure behaviour

A violation is a `FAIL` finding under the category `budget`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_budget` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/02_doctor_basic.sh` proves it, and CI runs that case.
{% endraw %}
