---
schema: post-mortem/v1
id: pm-20261013-case-1097s-message-read-as-an-early-exit
kind: post-mortem
title: A failure message in case 1097 contained "exit 0" and case 413 read it as an early exit
description: Case 413 refuses a case that can leave with status 0 before its end; a message that only mentions an exit status trips the same pattern.
status: resolved
date: 2026-10-13
runs:
  - workflow: validate
    id: "38364727993"
    commit: ba3fd916b3
    jobs: ["suite-shard (1)", "suite"]
    step: "behavioural tests"
cause:
  class: test
  summary: Line 250 of case 1097 carried the words `(exit 0)` inside a failure message, and case 413 scans every case for a status-0 exit before the end.
components:
  - test:test/cases/1097_the_projection_is_applied_at_landing.sh
  - test:test/cases/413_a_skipped_case_is_not_a_proof.sh
signals:
  - id: case
    text: "FAIL 413_a_skipped_case_is_not_a_proof"
  - id: reason
    text: "a case leaves with status 0 before its end"
fixed_by: ["commit:93c9c21894", "pr:1079"]
prevention: ["test:test/cases/413_a_skipped_case_is_not_a_proof.sh"]
tags: [suite]
---

# A failure message in case 1097 contained "exit 0" and case 413 read it as an early exit

## What failed

`suite-shard (1)` reported `FAIL 413_a_skipped_case_is_not_a_proof` in one second, naming
`test/cases/1097_the_projection_is_applied_at_landing.sh:250: grep -qx 'owed=no' "$OUT" ||
fail "a clean reading (exit 0) did not say owed=no: ..."`.

## Root cause

Case 413 holds that no case leaves with status 0 before its end, and finds candidates by
text. The message on line 250 describes an exit status in words; it is not an exit. Case
1097 was written and landed without case 413 being run over it.

## Fix

`93c9c21894` (pull request 1079) rewords the message.

## Prevention

Case 413 runs in a second and is the guard; a new case runs it before it is pushed.
