+++
title = "Exit codes are a contract, and no code means \"warn and continue\""
description = "Every command exits with one of a fixed set of codes: 0 ok, 2 usage, 10 contract unmet, 11 drift found, 12 missing artifact, 13 internal error, 15 refused. There is deliberately no code that means \"something was wrong but carry on\". A caller — including a git hook — must propagate them, and doctor scans hook lines for || true and || exit 0 around Majordomus invocations."
weight = 18
[extra]
claim_id = "exit-code-contract"
status = "guaranteed"
source = "docs/claims/exit-code-contract.md"
+++
{% raw %}

## What it means

Every command exits with one of a fixed set of codes: `0` ok, `2` usage, `10` contract unmet, `11` drift found, `12` missing artifact, `13` internal error, `15` refused. There is deliberately no code that means "something was wrong but carry on". A caller — including a git hook — must propagate them, and `doctor` scans hook lines for `|| true` and `|| exit 0` around Majordomus invocations.

## How it works

The codes are constants in `lib/common.sh`; every command exits through them. Read-only commands distinguish a failing finding (`10`) from informational drift (`11`) so that a hook can block on the first and a script can notice the second without confusing them. `finish --check` exits `0` when no task is active, so a pre-push hook never blocks a repository with nothing to enforce.

## How to see it

```bash
majordomus doctor; echo $?      # 0 healthy, 10 failures, 12 something missing
majordomus watch;  echo $?      # 0 no drift, 11 drift
```

## What it does not cover

Majordomus cannot make a caller honour the codes; `doctor`'s hook scan catches the two common ways of ignoring them, not every possible way.

## Why it exists

The source environment's enforcement contract had to spell out, in writing, that there is no "warn and continue", "soft fail" or "advisory block" — because hooks kept being written with `|| true`, and one documented fix replaced `|| true` with an explicit `set +e` block "with the same behaviour and no flagged pattern".
{% endraw %}
