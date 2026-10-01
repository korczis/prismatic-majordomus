+++
title = "Policy integrity"
description = "Policy integrity"
weight = 33
[extra]
kind = "rule"
slug = "majordomus-policy-integrity-1"
identity = "majordomus.policy-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/policy-integrity.v1.md"
+++
{% raw %}

## Rationale

The policy and every profile parse, declare version 1, and carry no key the schema does not define.

## Required behaviour

The policy and every profile parse, declare a supported version, and carry no key the schema does not define.

## Failure behaviour

A violation is a `FAIL` finding under the category `policy`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_policy` decides it, dispatched from `doctor, watch`. The behavioural case `test/cases/02_doctor_basic.sh` proves it, and CI runs that case.
{% endraw %}
