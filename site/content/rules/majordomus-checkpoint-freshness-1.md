+++
title = "Checkpoint freshness"
description = "Checkpoint freshness"
weight = 6
[extra]
kind = "rule"
slug = "majordomus-checkpoint-freshness-1"
identity = "majordomus.checkpoint-freshness@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/checkpoint-freshness.v1.md"
+++
{% raw %}

## Rationale

A task whose last checkpoint is older than its profile's interval is reported, not stopped.

## Required behaviour

A task whose newest checkpoint is older than its profile's interval is reported as stale, and the report does not stop the work.

## Failure behaviour

A violation is a `WARN` finding under the category `checkpoint`; the command continues and exits as it otherwise would. Under `watch` it is reported as drift and the command exits 11.

## Verification

`mj_validate_checkpoint` decides it, dispatched from `check, watch`. The behavioural case `test/cases/04_start_check.sh` proves it, and CI runs that case.
{% endraw %}
