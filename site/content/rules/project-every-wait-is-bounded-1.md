+++
title = "Every wait is bounded"
description = "Every wait is bounded"
weight = 83
[extra]
kind = "rule"
slug = "project-every-wait-is-bounded-1"
identity = "project.every-wait-is-bounded@1"
status = "active"
source = ".ai/repo/rules/project/every-wait-is-bounded.v1.md"
+++
{% raw %}

## Rationale

A worker that has started something and is waiting for it has three ways to be wrong and only
one of them looks like failure. It can wait for something that already finished — the result
sat unconsumed while the worker sat idle, which is what the terminal-focus stall of 2026-09-10
did four times in one evening. It can wait for something that died — the child is gone, no
result will ever arrive, and the wait has become permanent. Or it can wait for something that
is genuinely still working, which is the only case where waiting is the right thing to do.

From inside the wait these three are indistinguishable, because all three produce the same
observation: nothing new. **The absence of output is not evidence about any of them.** A worker
that treats quiet as progress will sit through the first two cases forever, and this repository
has watched it happen: a derivation whose second stage prints nothing for minutes reads exactly
like a run that has been interrupted, and an agent that had already been killed read, for some
time, exactly like an agent that was thinking.

The repository already refuses the same reasoning about verdicts — a check that has not
reported is unknown, never passing. That rule governs how a *finished* run's absent verdict is
read. This one governs the live case: what a worker must have arranged *before* it starts
waiting, so that quiet becomes a question it can answer instead of a state it can only endure.

The arrangement is cheap and it has to be made in advance. Once a worker is blocked on a
foreground command with no timeout, there is no longer anywhere to put the recovery.

## Required behaviour

**Every wait carries three things**: a timeout, so it ends; a completion predicate over real
state, so it can tell finished from quiet; and a recovery path, so that reaching the timeout
leads somewhere other than a stop.

**Nothing is spawned without a completion condition.** A subprocess, background worker,
delegate, test suite, build or external command is started together with the answer to how its
completion will be observed — an exit status, a file, a marker, a message. Retain the
identifier that makes that possible: a pid, a run id, a task id, an output path.

**Silence is inspected, not interpreted.** Where an operation produces no observable progress
within the interval expected of it, the worker looks: at the process and its children, at the
output and log files, at the state artifacts. It establishes whether the work is progressing,
finished, or dead, and it acts on what it found. In particular it checks whether the result it
is waiting for has already arrived and is simply unconsumed, because that is the cheapest of
the three to fix and the easiest to miss.

**No indefinite foreground wait.** A command that can block is given a timeout or run under
something that supervises it. Output is captured rather than left to a terminal, so that the
question "what did it say" survives the wait.

## Failure behaviour

Advisory today, and deliberately so: the mechanical half is a scan for the unbounded forms in
this repository's own shell and CI, which does not exist yet. ADR 0039 records it as the
follow-up that would make this rule blocking. Until then a violation is caught by review, and
by the worker that gets stuck.

## Verification

Review. Once the follow-up gate exists, the scan over `scripts/`, `test/` and
`.github/workflows/` for a spawn with no completion condition and a blocking command with no
timeout is what decides this rule.
{% endraw %}
