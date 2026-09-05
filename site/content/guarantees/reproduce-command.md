+++
title = "Every finding carries the command that reproduces it"
description = "When doctor, check, watch or finish report a failure or a drift, the line ends with [reproduce: <command>] — a shell command a person can run to see the same fact without Majordomus. A finding that cannot be reproduced independently is treated as a bug in Majordomus."
weight = 37
[extra]
claim_id = "reproduce-command"
status = "guaranteed"
source = "docs/claims/reproduce-command.md"
+++
{% raw %}

## What it means

When `doctor`, `check`, `watch` or `finish` report a failure or a drift, the line ends with `[reproduce: <command>]` — a shell command a person can run to see the same fact without Majordomus. A finding that cannot be reproduced independently is treated as a bug in Majordomus.

## How it works

Every finding goes through one printer in `lib/common.sh` that takes a level, a category, a subject, a message and a reproduce command. With `--json` the same fields come out as one JSON object per line, so a script can act on them.

## How to see it

```bash
majordomus doctor
# FAIL wiring      finish-on-push — hook .git/hooks/pre-push does not exist  [reproduce: ls -l .git/hooks/pre-push]
majordomus --json doctor | head -1
```

## What it does not cover

Informational lines (`INFO`) may omit a reproduce command when there is nothing to reproduce.

## Why it exists

Every audit in the source environment — including the one that cleaned up the others — contained at least one false claim caught only by an independent second reader. A finding you can rerun yourself does not need to be believed.
{% endraw %}
