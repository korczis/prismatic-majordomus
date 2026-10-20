---
schema: knowledge/v1
id: a-test-consistent-with-the-code-is-not-a-test-of-the-contract
kind: knowledge
class: lesson
title: "A test derived from current behaviour proves the code agrees with itself, not that it meets its contract"
description: "When writing or judging tests for a rule, a gate or a session record"
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - enforcement-evidence
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:test/cases/63_session_records.sh
relations:
  - type: relates_to
    target: file:.ai/repo/project/issues/I3146.yaml
  - type: relates_to
    target: file:.ai/repo/project/intents/a-session-record-says-what-it-served.yaml
  - type: relates_to
    target: file:.ai/repo/project/milestones/a-test-fails-when-the-code-is-wrong.yaml
---

# A test derived from current behaviour proves the code agrees with itself, not that it meets its contract

On 2026-09-11 the session-record case proved that a body sent on stdin is stored. Hook tests proved open, close and handover. Neither proved that a provider-created session carries a useful summary, so coverage was full on the wrong thing. On 2026-10-15 an audit found a documented test asserting Ok on a failed write: consistent with the code, contrary to the required contract. Tests come from acceptance criteria and failure models.

## Sources

- `chatgpt-majordomus:c116#3`
- `chatgpt-majordomus:c268#2`
