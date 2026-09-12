+++
title = "Handover integrity"
description = "Handover integrity"
weight = 21
[extra]
kind = "rule"
slug = "majordomus-handover-integrity-1"
identity = "majordomus.handover-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/handover-integrity.v1.md"
+++
{% raw %}

## Rationale

The resolver runs and reports either the record for this worktree and branch or its clean absence; a malformed record is never silently skipped, and a record describing a history this checkout no longer has is reported.

## Required behaviour

The record resolver reports the handover for this worktree and branch or its clean absence; a malformed record is never skipped silently.

## Failure behaviour

A violation is a `FAIL` finding under the category `resolver`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_resolver` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/23_context.sh` proves it, and CI runs that case.
{% endraw %}
