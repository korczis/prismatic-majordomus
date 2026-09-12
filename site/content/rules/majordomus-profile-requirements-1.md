+++
title = "Profile requirements"
description = "Profile requirements"
weight = 34
[extra]
kind = "rule"
slug = "majordomus-profile-requirements-1"
identity = "majordomus.profile-requirements@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/profile-requirements.v1.md"
+++
{% raw %}

## Rationale

A profile may demand more than the shared contract — a regression test, a decision record — and finish refuses without it.

## Required behaviour

A profile may require more than the shared contract, and finish refuses a completed outcome that lacks it.

## Failure behaviour

A violation is a `FAIL` finding under the category `regression`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_profile_requirements` decides it, dispatched from `finish`. The behavioural case `test/cases/16_profiles.sh` proves it, and CI runs that case.
{% endraw %}
