+++
title = "A failure message in case 1097 contained \"exit 0\" and case 413 read it as an early exit"
description = "A failure message in case 1097 contained \"exit 0\" and case 413 read it as an early exit"
weight = 4
[extra]
kind = "post-mortem"
slug = "pm-20261013-case-1097s-message-read-as-an-early-exit"
identity = "pm-20261013-case-1097s-message-read-as-an-early-exit"
status = "resolved"
source = ".ai/repo/post-mortems/pm-20261013-case-1097s-message-read-as-an-early-exit.md"
+++
{% raw %}

## A failure message in case 1097 contained "exit 0" and case 413 read it as an early exit

### What failed

`suite-shard (1)` reported `FAIL 413_a_skipped_case_is_not_a_proof` in one second, naming
`test/cases/1097_the_projection_is_applied_at_landing.sh:250: grep -qx 'owed=no' "$OUT" ||
fail "a clean reading (exit 0) did not say owed=no: ..."`.

### Root cause

Case 413 holds that no case leaves with status 0 before its end, and finds candidates by
text. The message on line 250 describes an exit status in words; it is not an exit. Case
1097 was written and landed without case 413 being run over it.

### Fix

`93c9c21894` (pull request 1079) rewords the message.

### Prevention

Case 413 runs in a second and is the guard; a new case runs it before it is pushed.
{% endraw %}
