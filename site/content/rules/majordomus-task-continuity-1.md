+++
title = "Task continuity"
description = "Task continuity"
weight = 52
[extra]
kind = "rule"
slug = "majordomus-task-continuity-1"
identity = "majordomus.task-continuity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/task-continuity.v1.md"
+++
{% raw %}

## Rationale

A task finished as partial or blocked should leave a handover record, not only a note section.

## Required behaviour

A task finished as partial or blocked leaves a handover record, not only a note section.

## Failure behaviour

A violation is a `WARN` finding under the category `continuity`; the command continues and exits as it otherwise would. Under `watch` it is reported as drift and the command exits 11.

## Verification

`mj_validate_continuity` decides it, dispatched from `finish`. The behavioural case `test/cases/25_continuity_lifecycle.sh` proves it, and CI runs that case.
{% endraw %}
