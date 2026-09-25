+++
title = "Sessions are workers, not memory"
description = "Sessions are workers, not memory"
weight = 50
[extra]
kind = "rule"
slug = "majordomus-sessions-are-workers-1"
identity = "majordomus.sessions-are-workers@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/principle-01-sessions-are-workers.v1.md"
+++
{% raw %}

## Rationale

A session ends and takes everything it merely remembered with it. The environments this tool was distilled from kept gigabytes of session notes standing in for a database, recovered by a hand-written runbook. State that survives is state that was written down deliberately, by a command, where the next worker reads it.

## Required behaviour

Treat every session as a worker that reads state from records and writes state to records; nothing that only a conversation remembers is state.

## Failure behaviour

Nothing decides this rule directly, so a violation is reported by nobody; it is the rules that depend on it that decide what a machine can decide and stop or warn accordingly.

## Verification

The enforced rules whose `depends_on` names this one carry the validators, the tests and the CI wiring. `majordomus doctrine list` shows them.
{% endraw %}
