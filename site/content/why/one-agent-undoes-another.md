+++
title = "One worker undoing another worker's change"
description = "A worker sees an unexplained change in its path, judges it wrong, and removes it — correctly, on the evidence it had."
weight = 130
[extra]
id = "one-agent-undoes-another"
status = "stable"
source = ".ai/repo/why/moments/one-agent-undoes-another.md"
+++
{% raw %}

## The moment

A worker adds a guard for a case it found the hard way. An hour later a second worker,
tidying the same function, deletes it: there is no test for it, no comment on it, and it
looks like defensive noise. Both changes are reasonable. One of them is a regression.

## Why it happens

The second worker had no way to learn that the first change was deliberate. An edit with no
recorded reason is indistinguishable from an accident, and a capable worker cleaning up a
file will remove what looks accidental. Isolation does not help here: the change had already
landed, so both workers were looking at the same tree.

## Why a better model does not fix it

A stronger second worker is *more* likely to remove the guard, not less — it will be more
confident that the code path is unreachable, and it is reasoning from the same absent
evidence. What was missing is the sentence saying why the guard exists.

## What it costs

The defect returns, and it returns silently, because the change that reintroduced it looks
like a cleanup. Recovering costs the original investigation a second time, plus the archaeology
to work out which of two plausible changes was the wrong one.

## What Majordomus does

Two mechanisms meet here. Scope makes concurrent ownership visible: `majordomus start`
requires the paths a task may touch, and another worktree holding an overlapping scope is
reported at the moment work begins. Decisions make past intent legible: `decision add`
refuses a decision with no `--why`, records the rejected alternative, and computes the task
and head from git. The next worker is assembled with the recent decisions before it starts.

## Before and after

```text
before   A: + if (session == null) return;      (reason: in A's head)
         B: - if (session == null) return;      (reason: looks unreachable)

after    B: majordomus start ... --scope lib/session
            OVERLAP ../checkout-a holds lib/session (task t-0091, active)
            decisions: "the guard covers the legacy mobile callback" — t-0091
```

## What it does not do

It does not lock files and it does not block a change. Whether a guard should stay is a
judgement; the tool's job is to make sure the judgement is made with the reason in view.
{% endraw %}
