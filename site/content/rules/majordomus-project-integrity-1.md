+++
title = "Project model integrity"
description = "Project model integrity"
weight = 35
[extra]
kind = "rule"
slug = "majordomus-project-integrity-1"
identity = "majordomus.project-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/project-integrity.v1.md"
+++
{% raw %}

## Rationale

Every milestone and issue file parses, carries the id its filename claims, and contains no key nobody reads.

## Required behaviour

Every milestone and issue file parses, carries the id its filename claims, and contains no key nobody reads.

## Failure behaviour

A violation is a `FAIL` finding under the category `project`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_project` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/44_model_doctrine.sh` proves it, and CI runs that case.
{% endraw %}
