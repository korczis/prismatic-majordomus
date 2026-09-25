+++
title = "Handovers transfer state, not transcripts"
description = "Handovers transfer state, not transcripts"
weight = 22
[extra]
kind = "rule"
slug = "majordomus-handovers-carry-state-1"
identity = "majordomus.handovers-carry-state@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/principle-10-handovers-carry-state.v1.md"
+++
{% raw %}

## Rationale

A transcript is the past as one worker experienced it. The next worker needs the present as the repository has it. A record with required sections and computed identity fields is the difference.

## Required behaviour

Write a handover as the current state and the next action, with computed front matter, and never paste a transcript, a diff or a narrative of the session into any record.

## Failure behaviour

Nothing decides this rule directly, so a violation is reported by nobody; it is the rules that depend on it that decide what a machine can decide and stop or warn accordingly.

## Verification

The enforced rules whose `depends_on` names this one carry the validators, the tests and the CI wiring. `majordomus doctrine list` shows them.
{% endraw %}
