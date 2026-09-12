+++
title = "Every public command is exercised and refuted"
description = "Every public command is exercised and refuted"
weight = 7
[extra]
kind = "rule"
slug = "majordomus-command-coverage-1"
identity = "majordomus.command-coverage@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/command-coverage.v1.md"
+++
{% raw %}

## Rationale

Test coverage that is remembered goes stale the day a command is added. Coverage that is
computed from the surface cannot: a command added to the registry is owed a behavioural
case and a negative case from that moment, and the maintainer who adds one without them is
told which layer is missing rather than discovering it later.

## Required behaviour

A case declares the commands it exercises with `# majordomus-covers:` and the commands
whose failure modes it asserts with `# majordomus-negative:`. Coverage is declared rather
than inferred, because assertions that pipe stdin or loop over a variable are invisible to
any scan of the source. Every public command must appear in at least one case of each
layer, and a header naming a command that is not public is a broken reference.

## Failure behaviour

A violation is a `FAIL` finding under the category `command`, and the command that found it
exits 10. Under `watch` the same violation is reported as drift and the command exits 11.
This rule applies only in the repository that carries the suite; an installation without
one reports the rule as skipped.

## Verification

`mj_validate_command_coverage` decides it, dispatched from `doctor, watch`. The behavioural
case `test/cases/31_command_coverage.sh` proves it, and CI runs that case.
{% endraw %}
