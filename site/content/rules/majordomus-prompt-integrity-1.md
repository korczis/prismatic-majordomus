+++
title = "Prompt integrity"
description = "Prompt integrity"
weight = 39
[extra]
kind = "rule"
slug = "majordomus-prompt-integrity-1"
identity = "majordomus.prompt-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/prompt-integrity.v1.md"
+++
{% raw %}

## Rationale

Every repository-local prompt asset renders, and every token in it is one the renderer knows.

## Required behaviour

Every reusable prompt asset renders, and every token in it is one the renderer knows.

## Failure behaviour

A violation is a `FAIL` finding under the category `prompt`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_prompts` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/24_prompt_search.sh` proves it, and CI runs that case.
{% endraw %}
