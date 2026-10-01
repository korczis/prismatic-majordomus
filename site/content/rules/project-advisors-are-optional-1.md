+++
title = "Advisors are optional; reasoning, governance and CI never depend on one"
description = "Advisors are optional; reasoning, governance and CI never depend on one"
weight = 63
[extra]
kind = "rule"
slug = "project-advisors-are-optional-1"
identity = "project.advisors-are-optional@1"
status = "active"
source = ".ai/repo/rules/project/advisors-are-optional.v1.md"
+++
{% raw %}

## Rationale

The owner's review protocol was a sequence of vendor names in one worker's private notes.
The first time it was run through code, in the session that built ADR 0098, one advisor's
credential was a placeholder and another refused to run in an untrusted directory. A
workflow written as "ask this one, then that one" would have stopped there. Recorded as
`auth_failed` and `error`, the two failures cost one bounded wait each, and the work went
on.

## Required behaviour

1. **Capabilities, not names.** Reasoning code, its capability, its command, its Cockpit
   page and the transport's contract and driver name advisory capabilities. Which advisor
   provides one is `share/advisors.yaml`'s answer, in its declaration order.
2. **Absence is a status.** An advisor is `available`, `unavailable`, `not_configured`,
   `disabled`, `temporarily_failed` or `rate_limited`, each with its reason. None of these
   is a failure of the repository; `doctor` reports them as information.
3. **No advisor is a plan.** Material uncertainty with no suitable advisor gets the
   structured local review, and the session concludes, implements and validates.
4. **Failures are bounded and recorded.** Every exchange has a deadline; its outcome is a
   record; repeated failures open a circuit that closes after its cooldown.
5. **Nothing required calls a model.** Entry, `doctor`, generation, validation and CI make
   no model call, and no workflow or gate names a model credential.

## Failure behaviour

`majordomus reasoning check` exits 10 on provider-independent code naming an advisor, an
adapter the catalogue names with no module, a catalogue reference that resolves to nothing,
or CI naming a model credential; `doctor` fails on the same findings.
{% endraw %}
