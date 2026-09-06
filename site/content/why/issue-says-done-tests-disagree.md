+++
title = "The issue says done and the repository disagrees"
description = "Completion is recorded as a state change in a tracker rather than as evidence in the repository, so the two drift immediately."
weight = 220
[extra]
id = "issue-says-done-tests-disagree"
status = "stable"
source = ".ai/repo/why/moments/issue-says-done-tests-disagree.md"
+++
{% raw %}

## The moment

The issue is closed. Two of its four acceptance criteria were never implemented, and the
evidence field says "tested locally". Nobody lied; the issue was closed by someone who
believed the work was finished, and nothing was in a position to disagree.

## Why it happens

Closing is a state change in a tracker, and the tracker knows nothing about the repository.
The acceptance criteria are prose in a field, so checking them is a reading, and the reading
is done by the person most convinced the work is complete.

## Why a better model does not fix it

The worker was not asked to check the criteria; it was asked to do the work, and it did.
Asking a worker to also assess its own completion returns a confident assessment, which is
the input we already had.

## What it costs

The gap between recorded and actual completion is invisible until something downstream
fails. Meanwhile the plan is used to decide what to start next, so work begins on top of
foundations that were only reported as laid.

## What Majordomus does

No status is stored anywhere. `READY`, `BLOCKED`, `ACTIVE`, `VERIFY` and `DONE` are derived
from the events an issue recorded about itself and from the state of its dependencies, every
time the plan is read; a hand-written status field is an unknown key. `plan evidence` refuses
narrative — it needs a command or an artifact, and it records the result with the commit.
`plan done` refuses while any declared evidence is uncovered or a dependency is not done.
`finish` applies the same discipline to the task: nothing is written while a line of the
contract fails.

## Before and after

```text
before   tracker: closed          repository: two criteria unimplemented

after    $ majordomus plan done I0042
         refused: evidence 'behaviour_tested' is uncovered
                  evidence must name a command or an artifact
```

## What it does not do

It does not decide whether the evidence is good evidence. It refuses evidence that is not a
command or an artifact, records what the command actually returned, and will not derive
`DONE` while a declared requirement is uncovered.
{% endraw %}
