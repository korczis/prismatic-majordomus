+++
title = "Dependency graph integrity"
description = "Dependency graph integrity"
weight = 12
[extra]
kind = "rule"
slug = "majordomus-dag-integrity-1"
identity = "majordomus.dag-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/dag-integrity.v1.md"
+++
{% raw %}

## Rationale

The issue dependency graph is acyclic, every edge names an issue that exists, and no issue is executing ahead of a dependency that is not done.

## Required behaviour

The issue dependency graph is acyclic, every edge names an issue that exists, and no issue executes ahead of a dependency that is not done.

## Failure behaviour

A violation is a `FAIL` finding under the category `dag`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_dag` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/44_model_doctrine.sh` proves it, and CI runs that case.
{% endraw %}
