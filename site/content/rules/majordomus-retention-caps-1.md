+++
title = "Retention caps"
description = "Retention caps"
weight = 43
[extra]
kind = "rule"
slug = "majordomus-retention-caps-1"
identity = "majordomus.retention-caps@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/retention-caps.v1.md"
+++
{% raw %}

## Rationale

The ledger and the handover directory stay under the caps the policy sets, so durable state does not grow without bound.

## Required behaviour

The ledger, the handovers and the checkpoints stay under the caps the policy sets.

## Failure behaviour

A violation is a `FAIL` finding under the category `retention`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_retention` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/02_doctor_basic.sh` proves it, and CI runs that case.
{% endraw %}
