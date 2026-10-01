+++
title = "Enforcement wiring"
description = "Enforcement wiring"
weight = 19
[extra]
kind = "rule"
slug = "majordomus-enforcement-wiring-1"
identity = "majordomus.enforcement-wiring@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/enforcement-wiring.v1.md"
+++
{% raw %}

## Rationale

Every enforcement the policy declares is invoked by the hook it names, from a file the dispatcher will actually execute, without swallowing the exit code.

## Required behaviour

Every enforcement the policy declares is invoked by the hook it names, from a file the dispatcher executes, without a swallowed exit code.

## Failure behaviour

A violation is a `FAIL` finding under the category `wiring`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_wiring` decides it, dispatched from `doctor`. The behavioural case `test/cases/14_wiring_dispatcher.sh` proves it, and CI runs that case.
{% endraw %}
