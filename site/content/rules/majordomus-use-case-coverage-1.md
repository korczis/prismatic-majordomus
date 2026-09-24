+++
title = "Use-case coverage"
description = "Use-case coverage"
weight = 54
[extra]
kind = "rule"
slug = "majordomus-use-case-coverage-1"
identity = "majordomus.use-case-coverage@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/use-case-coverage.v1.md"
+++
{% raw %}

## Rationale

A capability nobody has written a use case for is a capability nobody has shown a person
performing, and a use case with no scenario is prose. Coverage computed from the registry
and the use cases, with the policy saying which classes must be covered, turns "we should
document this" into a line `doctor` prints and `finish` refuses on.

## Required behaviour

For every public command of the command registry, every guaranteed claim with a
responsibility, and every MCP tool the executable projects, `majordomus usecase coverage`
counts the active use cases that name it and the ones whose scenario runs it. The policy's
`use_cases.coverage` says, per class, whether a gap is `required` (a failure), `advisory`
(reported) or `off`. A draft use case never counts.

## Failure behaviour

A violation is a `FAIL` finding under the category `use-case` naming the capability, the
counts and the scaffold command, and the command that found it exits 10; under `finish`
the policy key `use_cases_covered` refuses completion.

## Verification

`mj_validate_use_case_coverage` decides it, dispatched from `doctor, check, finish`. The
behavioural case `test/cases/94_use_cases.sh` proves it, and CI runs that case.
{% endraw %}
