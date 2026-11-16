+++
title = "A slow test is made cheaper, never given a longer timeout"
description = "A slow test is made cheaper, never given a longer timeout"
weight = 1
[extra]
kind = "rule"
slug = "majordomus-a-slow-test-is-made-cheaper-1"
identity = "majordomus.a-slow-test-is-made-cheaper@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/a-slow-test-is-made-cheaper.v1.md"
+++
{% raw %}

## Rationale

A test is sized against the runner it was written on. When the same test is judged on a
slower or busier runner, the margin it left is gone, and it times out. The time-out says
nothing about the code under test, and it blocks every change behind it.

Observed in algoglyph, a public repository: tests sized against fast hosted CI timed out at
120 s on a loaded self-hosted runner and blocked deploys. Two fixes are easy and both are
wrong. Raising the timeout hides the cost until the next runner is slower still. Loosening a
statistical band until the smaller run passes turns a test that could fail into one that
cannot, and nothing in its output says so.

## Required behaviour

- **Well inside the limit, on the slowest judge.** A test's limit is set by the slowest
  runner whose verdict counts: a self-hosted or shared runner under load, not a developer's
  machine. Stay well inside it there. About 20 s locally is a working target for a single
  test, because a loaded runner can take several times as long.
- **The runtime is recorded.** A test's runtime is measured and kept where a reviewer can
  read it: the case's header, the suite's timing report, or the CI step's own timing. A
  claim that a test is fast is a measurement with its machine and date, not a belief.
- **A slow test is made cheaper.** Reduce the work: fewer frames, seeds, particles,
  iterations or samples. Do this only where both of these hold at the new size:
  - the statistical band the test asserts is derived again for that size (the tolerance of
    a mean over fewer samples is wider, by the formula the band was derived from, not by a
    guess), and the reduced test still passes inside it;
  - a planted mutation of the code under test still fails the reduced test
    (`majordomus.an-equivalence-proof-keeps-a-failing-mutation@1` says how such a control
    is kept).
- **Otherwise it is split.** When no smaller size keeps both, the test is split into parts
  that each stay inside the limit, or the expensive part moves to a scheduled run that
  states it does not gate the change.
- **Not a fix:** raising a timeout, retrying until green, or widening a band by hand to
  make a smaller run pass. A timeout is raised only when the slowest judge itself changed,
  and the change records that runner, its measured time and why.

## Failure behaviour

Nothing decides this rule directly: it has no validator, so a violation is reported by
nobody. It is normative text. A worker applies it when a test is slow or times out, and a
reviewer refuses a change whose only answer to a timeout is a longer timeout or a wider
band.

## Verification

For a test that was made cheaper, the change shows the runtime before and after, with the
machine; the band derived again for the new size, with its derivation; and a planted
mutation failing the reduced test. For a test that was split, each part's runtime is shown.
A repository may add a project rule with a validator that decides part of this
mechanically, for example a ceiling on recorded case durations.
{% endraw %}
