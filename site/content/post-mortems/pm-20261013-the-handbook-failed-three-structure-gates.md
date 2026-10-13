+++
title = "The handbook sources failed the design, ui-check and lease-reader gates on the trunk"
description = "The handbook sources failed the design, ui-check and lease-reader gates on the trunk"
weight = 6
[extra]
kind = "post-mortem"
slug = "pm-20261013-the-handbook-failed-three-structure-gates"
identity = "pm-20261013-the-handbook-failed-three-structure-gates"
status = "resolved"
source = ".ai/repo/post-mortems/pm-20261013-the-handbook-failed-three-structure-gates.md"
+++
{% raw %}

## The handbook sources failed the design, ui-check and lease-reader gates on the trunk

### What failed

`structure` ended with `run-plan: 53 gate(s) ran in 972s; failed: ui-check lease-reader
design`, and case `110_lease_reader` failed in shard 2. The session that read the job log
that night counted ninety findings of `a colour chosen outside share/design/tokens.yaml`,
all under `handbook/cs/`.

### Root cause

The handbook's sources were brought into the repository as they had been written outside
it (ADR 0149): inline colours of their own, and scripts and prose the two other gates read
as a second user interface and a second reader of the lease. Whether the landing that
brought them had run the structure job on its merged tree was not established for this
record.

### Fix

Pull request 1062 (`61d4417675`) makes the handbook wear the declared design and clears the
three gates.

### Prevention

The three gates and case 110 are the guard. The structure job on the merged tree before a
pinned merge is the landing rule since that day.
{% endraw %}
