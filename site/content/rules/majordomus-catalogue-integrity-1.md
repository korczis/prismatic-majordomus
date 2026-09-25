+++
title = "Catalogue integrity"
description = "Catalogue integrity"
weight = 5
[extra]
kind = "rule"
slug = "majordomus-catalogue-integrity-1"
identity = "majordomus.catalogue-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/catalogue-integrity.v1.md"
+++
{% raw %}

## Rationale

Every use case and application describes the tool in terms the tool has — each command, doctrine and claim it names exists, and the two catalogues reference each other in both directions.

## Required behaviour

Every use case and application names only commands, rules and claims that exist, and the two catalogues reference each other in both directions.

## Failure behaviour

A violation is a `FAIL` finding under the category `catalogue`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_catalogue` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/28_catalogue.sh` proves it, and CI runs that case.
{% endraw %}
