+++
title = "A change whose provenance cannot be reconstructed"
description = "Attribution stops at the commit, so what authorised a change, what verified it and what governed it are unrecoverable."
weight = 380
[extra]
id = "who-did-this-and-under-what-policy"
status = "stable"
source = ".ai/repo/why/moments/who-did-this-and-under-what-policy.md"
+++
{% raw %}

## The moment

"Who made this change, what authorised it, and what verified it?" The available answer is a
commit with a human author, a message written by a worker, and a merged pull request with
one approval. None of those is the answer to any of the three questions.

## Why it happens

Git records authorship of a patch, which was a good proxy while a person typed every line.
It is no longer: the author field records who ran the worker, and everything else that
matters — the objective, the boundary, the policy in force, the verification that ran — is
either in a conversation or nowhere.

## Why a better model does not fix it

There is no inference that recovers what was never recorded. Asking a model to reconstruct
provenance from a diff produces a plausible story, which in an audit is worse than nothing.

## What it costs

In a regulated setting, the ability to demonstrate a control at all. Everywhere else, the
ability to answer the most common question in incident review: what did we think we were
doing, and what checked it.

## What Majordomus does

Identity fields are computed from git and never authored: a record that names a branch, a
head, a working tree or a set of changed files had those values computed at the moment it
was written, and a worker supplying them is an error rather than an override. A commit
carries the task that produced it. The task carries its objective, its declared scope and
the profile it ran under. The ledger carries the events — started, checkpointed, decided,
handed over, finished with a typed outcome, and the verification command with its exit code
and duration. The generated instruction files carry the hash of the policy that produced
them, so which rules were in force is a tracked fact.

## Before and after

```text
before   commit 8c31f0e  "fix auth"  author: a person

after    $ majordomus history --task t-20260903193012-a4f1
         task_started   objective="split the callback validation" profile=implementation
                        scope=lib/auth,test/auth  head=1f0a3c2
         decision       "normalise before comparing"  why=...  rejected=...
         task_finished  outcome=completed  verify="make test" exit=0 1m12s  head=8c31f0e
```

## What it does not do

It does not identify which model or vendor produced a change, and it does not sign anything.
It records what this repository decided, bounded, ran and accepted, with the identity fields
computed rather than asserted.
{% endraw %}
