+++
title = "Escalate capability and effort only when justified"
description = "Escalate capability and effort only when justified"
weight = 24
[extra]
kind = "rule"
slug = "majordomus-justified-escalation-1"
identity = "majordomus.justified-escalation@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/principle-05-justified-escalation.v1.md"
+++
{% raw %}

## Rationale

Reasoning at maximum by default is a cost without a decision behind it. A profile is the decision; an escalation is a second decision that leaves a record.

## Required behaviour

Run at the profile's capability class and effort, and when you raise either, record that you did and why.

## Failure behaviour

Nothing decides this rule directly, so a violation is reported by nobody; it is the rules that depend on it that decide what a machine can decide and stop or warn accordingly.

## Verification

The enforced rules whose `depends_on` names this one carry the validators, the tests and the CI wiring. `majordomus doctrine list` shows them.
{% endraw %}
