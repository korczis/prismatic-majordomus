+++
title = "Verify outcomes, not activity"
description = "Verify outcomes, not activity"
weight = 56
[extra]
kind = "rule"
slug = "majordomus-verify-outcomes-1"
identity = "majordomus.verify-outcomes@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/principle-08-verify-outcomes.v1.md"
+++
{% raw %}

## Rationale

Activity is easy to report and impossible to check. An outcome that a command produced, with its exit code and duration recorded, is checkable by anyone later.

## Required behaviour

Accept a verification command's recorded exit code as evidence of completion, and accept nothing that merely describes verification.

## Failure behaviour

Nothing decides this rule directly, so a violation is reported by nobody; it is the rules that depend on it that decide what a machine can decide and stop or warn accordingly.

## Verification

The enforced rules whose `depends_on` names this one carry the validators, the tests and the CI wiring. `majordomus doctrine list` shows them.
{% endraw %}
