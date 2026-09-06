+++
title = "A task is started with a declared file scope and a profile, and only one task is active per checkout"
description = "majordomus start requires --scope: the repository paths this task may touch. It records the scope, the profile, the owner and the git state in state/current.yaml. While that record's outcome is active, a second start in the same checkout is refused with exit code 15. A task ends by finish or by handover --close; the next start archives the old record rather than discarding it."
weight = 39
[extra]
claim_id = "scoped-task"
status = "guaranteed"
source = "docs/claims/scoped-task.md"
+++
{% raw %}

## What it means

`majordomus start` requires `--scope`: the repository paths this task may touch. It records the scope, the profile, the owner and the git state in `state/current.yaml`. While that record's outcome is `active`, a second `start` in the same checkout is refused with exit code 15. A task ends by `finish` or by `handover --close`; the next `start` archives the old record rather than discarding it.

## How it works

`lib/start.sh` splits `--scope` on commas (repeated flags accumulate), normalises each path — no leading `./`, no trailing `/`, no `..`, no absolute paths — and refuses anything that escapes the repository. It writes the record atomically and appends `task.started` to the ledger. Scope is then read by `check` and `finish`, which normalise both sides again before comparing, so a hand-edited entry with a trailing slash still contains its files.

## How to see it

```bash
majordomus start "fix auth" --scope lib/auth/,./test/auth
# started t-…  profile=implementation  scope=lib/auth,test/auth
majordomus start "second" --scope lib/other
# majordomus: task t-… is active ('fix auth'); run majordomus handover or majordomus finish first
```

## What it does not cover

One task per checkout is enforced; one checkout per concurrent writer is advice. Two sessions sharing one checkout will block each other's pushes until scopes are declared — which is what happened while this site was built, and why the design asks for a worktree per writer.

## Why it exists

In the source environment the scope declaration was an optional second step after creating a worktree; ten of eighteen active worktrees had never declared one, which made the blocking check unreachable in the majority case. If a step can be skipped it will be; here the declaration is part of `start`.
{% endraw %}
