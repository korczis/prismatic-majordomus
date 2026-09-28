+++
title = "AI layer integrity"
description = "AI layer integrity"
weight = 2
[extra]
kind = "rule"
slug = "majordomus-ai-layout-integrity-1"
identity = "majordomus.ai-layout-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/ai-layout-integrity.v1.md"
+++
{% raw %}

## Rationale

The repository's AI layer is real: the manifest declares a format this executable reads and every section it names exists, the checkout-local half is ignored by git and nothing under it is tracked, and no project data remains under the pre-.ai .majordomus/ path.

## Required behaviour

The repository's AI layer is real: the manifest declares a format this executable reads and names sections that exist, the local half is ignored by git with nothing under it tracked, and no project data remains under the pre-.ai path.

## Failure behaviour

A violation is a `FAIL` finding under the category `layout`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_ai_layout` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/01_init.sh` proves it, and CI runs that case.
{% endraw %}
