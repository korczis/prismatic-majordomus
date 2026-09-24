+++
title = "Scope integrity"
description = "Scope integrity"
weight = 47
[extra]
kind = "rule"
slug = "majordomus-scope-integrity-1"
identity = "majordomus.scope-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/scope-integrity.v1.md"
+++
{% raw %}

## Rationale

A task touches only the paths it claimed; work found elsewhere is not accepted as done.

## Required behaviour

Work outside the paths a task claimed is not accepted as that task's work.

## Failure behaviour

A violation is a `FAIL` finding under the category `scope`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_scope` decides it, dispatched from `check, finish, watch`. The behavioural case `test/cases/04_start_check.sh` proves it, and CI runs that case.
{% endraw %}
