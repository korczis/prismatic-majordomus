+++
title = "Define done before executing"
description = "Define done before executing"
weight = 15
[extra]
kind = "rule"
slug = "majordomus-define-done-first-1"
identity = "majordomus.define-done-first@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/principle-07-define-done-first.v1.md"
+++
{% raw %}

## Rationale

When done means the worker said so, every task is done. A contract written first is what makes a refusal possible, and a refusal is the whole point of supervision.

## Required behaviour

Know the contract a task must meet before starting it, and let the contract, not the worker's word, decide whether it is finished.

## Failure behaviour

Nothing decides this rule directly, so a violation is reported by nobody; it is the rules that depend on it that decide what a machine can decide and stop or warn accordingly.

## Verification

The enforced rules whose `depends_on` names this one carry the validators, the tests and the CI wiring. `majordomus doctrine list` shows them.
{% endraw %}
