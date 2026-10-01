+++
title = "Execution depth is not output verbosity"
description = "Execution depth is not output verbosity"
weight = 17
[extra]
kind = "rule"
slug = "majordomus-depth-is-not-verbosity-1"
identity = "majordomus.depth-is-not-verbosity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/principle-06-depth-is-not-verbosity.v1.md"
+++
{% raw %}

## Rationale

Long output is not evidence of careful work, and careful work does not need long output. Confusing the two produces reports nobody reads and depth nobody controls.

## Required behaviour

Keep the depth of work and the length of its report as two separate settings, each taken from the profile.

## Failure behaviour

Nothing decides this rule directly, so a violation is reported by nobody; it is the rules that depend on it that decide what a machine can decide and stop or warn accordingly.

## Verification

The enforced rules whose `depends_on` names this one carry the validators, the tests and the CI wiring. `majordomus doctrine list` shows them.
{% endraw %}
