+++
title = "Policy completeness"
description = "Policy completeness"
weight = 32
[extra]
kind = "rule"
slug = "majordomus-policy-completeness-1"
identity = "majordomus.policy-completeness@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/policy-completeness.v1.md"
+++
{% raw %}

## Rationale

Every policy value the code reads is declared in the skeleton policy, and no reader carries its own default for one.

## Required behaviour

Every policy value the code reads is declared in the skeleton policy, and no reader carries a default of its own.

## Failure behaviour

A violation is a `FAIL` finding under the category `policy`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_policy_defaults` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/28_no_hardcoded_values.sh` proves it, and CI runs that case.
{% endraw %}
