+++
title = "Note integrity"
description = "Note integrity"
weight = 29
[extra]
kind = "rule"
slug = "majordomus-note-integrity-1"
identity = "majordomus.note-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/note-integrity.v1.md"
+++
{% raw %}

## Rationale

Every outcome needs a note carrying the sections that outcome requires, and no transcript.

## Required behaviour

Every outcome needs a note carrying the sections that outcome requires, and never a transcript.

## Failure behaviour

A violation is a `FAIL` finding under the category `note`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_note` decides it, dispatched from `finish`. The behavioural case `test/cases/06_finish.sh` proves it, and CI runs that case.
{% endraw %}
