+++
title = "Ledger integrity"
description = "Ledger integrity"
weight = 26
[extra]
kind = "rule"
slug = "majordomus-ledger-integrity-1"
identity = "majordomus.ledger-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/ledger-integrity.v1.md"
+++
{% raw %}

## Rationale

Every line of the append-only ledger is a well-formed event; the one durable record nothing else can reconstruct stays readable.

## Required behaviour

Every line of the append-only ledger is a well-formed event.

## Failure behaviour

A violation is a `FAIL` finding under the category `records`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_ledger` decides it, dispatched from `check, doctor, watch`. The behavioural case `test/cases/22_history.sh` proves it, and CI runs that case.
{% endraw %}
