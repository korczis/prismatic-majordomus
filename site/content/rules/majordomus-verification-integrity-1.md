+++
title = "Verification integrity"
description = "Verification integrity"
weight = 55
[extra]
kind = "rule"
slug = "majordomus-verification-integrity-1"
identity = "majordomus.verification-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/verification-integrity.v1.md"
+++
{% raw %}

## Rationale

Completion requires a verification command that actually ran and exited 0; its exit code and duration are recorded.

## Required behaviour

A completed outcome requires a verification command that ran and exited zero, with its exit code and duration recorded.

## Failure behaviour

A violation is a `FAIL` finding under the category `verification`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_verification` decides it, dispatched from `finish`. The behavioural case `test/cases/19_end_to_end.sh` proves it, and CI runs that case.
{% endraw %}
