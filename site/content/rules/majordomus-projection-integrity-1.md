+++
title = "Projection integrity"
description = "Projection integrity"
weight = 36
[extra]
kind = "rule"
slug = "majordomus-projection-integrity-1"
identity = "majordomus.projection-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/projection-integrity.v1.md"
+++
{% raw %}

## Rationale

Every generated instruction file exists, matches the stamp it carries, and is never silently overwritten after a hand edit.

## Required behaviour

Every generated instruction file exists, matches the content it declares, and is never silently overwritten after a hand edit.

## Failure behaviour

A violation is a `FAIL` finding under the category `projection`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_projection` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/03_update.sh` proves it, and CI runs that case.
{% endraw %}
