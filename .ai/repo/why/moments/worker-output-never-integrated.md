---
schema: moment/v1
id: worker-output-never-integrated
kind: moment
title: 'Finished work that never reached the trunk'
short_title: 'Never integrated'
hook: 'found finished work on a branch that nobody ever merged'
summary: 'Workers produce far more than integration absorbs, and nothing distinguishes work that is done from work that is done and landed.'
status: stable
severity: high
frequency: common
weight: 150
audiences: [ai-native-team, engineering-lead, research-team]
areas: [coordination, work-tracking]
lifecycle: [review, operations]
tags: [integration, throughput, waste, parallelism]
signals:
  - id: more-produced-than-merged
    text: 'Far more work was produced this week than was integrated, and the gap is not tracked anywhere.'
  - id: done-but-unmerged
    text: 'Something reported as finished is sitting on a branch that nobody has looked at.'
  - id: no-landed-signal
    text: 'Nothing distinguishes "the worker finished" from "the change is in the trunk".'
examples:
  - id: eighty-branches
    audience: ai-native-team
    title: 'Eighty attempts, nine merges'
    before: 'Parallel workers produce dozens of complete changes; a fraction are reviewed and fewer land, and nothing records which.'
    after: 'A finish records a typed outcome and the verification that ran; what has not been finished, and what was finished as `partial`, are both visible.'
  - id: overnight-yield
    audience: engineering-lead
    title: 'Measuring the wrong thing'
    before: 'Throughput is reported as work produced, which is the number that flatters and the number that does not matter.'
    after: 'The ledger distinguishes started, handed over, and finished with an outcome, so the gap between produced and accepted is a readable figure.'
  - id: experiment-shelf
    audience: research-team
    title: 'The shelf of finished experiments'
    before: 'A promising branch is complete and never written up, so it is neither used nor discarded.'
    after: 'Its handover names the objective and the next action, and the outcome vocabulary has a value for "this worked and stopped here".'
commands: [finish, history, plan]
capabilities: [health.report, objects.list]
claims: [typed-outcome, finish-contract, history-ledger-read, evidence-gates-done]
doctrines: [majordomus.verification-integrity, majordomus.isolated-parallelism, majordomus.verify-outcomes, majordomus.project-integrity]
use_cases: [accept-or-refuse-finished-work, read-back-what-happened, deliver-issues-in-waves]
related: [abandoned-worktree, two-agents-one-bug, spend-not-tied-to-outcomes]
aliases: ['unmerged work', 'integration bottleneck', 'produced but not accepted']
---

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
