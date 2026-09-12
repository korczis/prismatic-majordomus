+++
title = "Layout integrity"
description = "Layout integrity"
weight = 25
[extra]
kind = "rule"
slug = "majordomus-layout-integrity-1"
identity = "majordomus.layout-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/layout-integrity.v1.md"
+++
{% raw %}

## Rationale

The directories the durable commands write into are installed rather than created on first use.

## Required behaviour

The directories the durable commands write into are installed rather than created on first use.

## Failure behaviour

A violation is a `WARN` finding under the category `layout`; the command continues and exits as it otherwise would. Under `watch` it is reported as drift and the command exits 11.

## Verification

`mj_validate_layout` decides it, dispatched from `doctor`. The behavioural case `test/cases/25_continuity_lifecycle.sh` proves it, and CI runs that case.
{% endraw %}
