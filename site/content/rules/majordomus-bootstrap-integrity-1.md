+++
title = "Bootstrap integrity"
description = "Bootstrap integrity"
weight = 4
[extra]
kind = "rule"
slug = "majordomus-bootstrap-integrity-1"
identity = "majordomus.bootstrap-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/bootstrap-integrity.v1.md"
+++
{% raw %}

## Rationale

The path from a human reader to the AI layer is unbroken — README.md names AGENTS.md, every generated instruction file points at .ai/README.md, and none of them carries a rule of its own.

## Required behaviour

A person reaches the AI layer through README.md and AGENTS.md, every generated instruction file points at .ai/README.md, and none of them carries a rule of its own.

## Failure behaviour

A violation is a `FAIL` finding under the category `bootstrap`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_bootstrap` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/03_update.sh` proves it, and CI runs that case.
{% endraw %}
