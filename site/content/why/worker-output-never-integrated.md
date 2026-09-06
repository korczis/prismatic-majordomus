+++
title = "Finished work that never reached the trunk"
description = "Workers produce far more than integration absorbs, and nothing distinguishes work that is done from work that is done and landed."
weight = 150
[extra]
id = "worker-output-never-integrated"
status = "stable"
source = ".ai/repo/why/moments/worker-output-never-integrated.md"
+++
{% raw %}

## The moment

A change is complete, tested and sitting on a branch. It has been there for two weeks. The
worker that wrote it reported success and stopped; nothing since has been obliged to notice
that the work exists.

## Why it happens

Producing work got cheap and integrating it did not. The bottleneck moved to review and
merge, and nothing measures the queue in front of that bottleneck, so the natural response
is to produce more. In the environment this tool was distilled from, eighty parallel efforts
produced nine merges.

## Why a better model does not fix it

Better workers widen the gap. The constraint is downstream of them: a human decision to
accept, and the review capacity behind it. Adding capability to the producing side makes the
unintegrated pile larger.

## What it costs

Every unmerged change decays — it conflicts more each day — so the work is not merely idle,
it is depreciating. And because the pile is invisible, the team plans as though the produced
work were delivered.

## What Majordomus does

Finishing is an event with a typed outcome, not a message: `completed`, `partial`,
`blocked`, `no_match`, `failed`, recorded with the verification command that ran, its exit
code and its duration. The ledger therefore distinguishes what was started, what was handed
over and what was accepted, and `history` reads the three back separately. Where the
repository carries a plan, an issue is not done until its declared evidence is covered, so
"finished by a worker" and "done" are different states with different gates.

## Before and after

```text
before   worker: "complete"          -> branch, indefinitely

after    $ majordomus history --since 7d --event task_finished
         t-…a4f1  completed  verify="make test" exit=0
         t-…b207  partial    "the backfill is written; the cutover is not"
```

## What it does not do

It does not merge, review or schedule integration. It makes the difference between produced
and accepted a fact the repository states rather than one a person estimates.
{% endraw %}
