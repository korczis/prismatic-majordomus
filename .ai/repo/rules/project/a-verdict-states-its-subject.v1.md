---
id: project.a-verdict-states-its-subject
version: 1
kind: rule
title: A verdict states what it is a verdict about
description: A check reports the size and the identity of the population it examined, never a bare count or an "every one" whose denominator is elsewhere; and a detector that greps for a literal may not count its own prose, its own fixtures or another language's builtin as its subject.
statement: A verdict names its denominator, and a detector distinguishes its subject from what merely looks like its subject.
status: active
class: blocking
depends_on: [project.never-reported-is-not-green@1, project.empty-is-not-failure@1, project.no-counts-in-prose@1]
tags: [evidence, governance, gates]

x-majordomus:
  tests: [test/cases/230_verdict_states_its_subject.sh]
---

# Rationale

`doctor` reported, in one run:

```
OK   doctrine    41 doctrines — validator, dispatch, propagation, test and CI resolve for every one
OK   rules       122 rule(s) — resolve in one deterministic order
```

Both sentences are true. Read together they say the repository is covered. What they
actually say is that 41 of 122 rules carry a validator and that those 41 are wired
correctly; about the other 81 the first line says nothing at all, and nothing in the
sentence tells the reader that. The doctrine surface also reported *missing validators: 0*,
which was true of its registry and silent about the 57 blocking rules that are not in the
registry — a rule with no validator cannot be missing one, because it was never counted.

This is the same defect as ADR 0041: a subsystem went quiet for six days while every health
check passed, because every check was looking at a subset that excluded it. A number that
does not carry its denominator cannot be wrong, which is why it is dangerous.

The second half of the rule is the detector's side of the same idea, and it arrived as three
gates in one day whose *measurement* was wrong rather than whose subject was bad:

- `scripts/ci/order-check` reported "unpinned shell sorts rose from 0 to 13". Eleven of the
  thirteen were **jq's `sort` builtin** — seven in a `.jq` program, four inside single-quoted
  jq filters in a shell case. Nobody can pin jq's sort with `LC_ALL=C`, so the ratchet could
  not be satisfied by anyone, and it failed every branch.
- `scripts/ci/lease-reader-check` reported **itself**. Its subject is a file that parses the
  shared server's lease; its detector is a grep for the lease's file name; and one of its own
  comments explains why the bare file name has to be matched. The gate counted its own
  documentation of its subject as its subject.
- `scripts/ci/pipefail-check` reported `lib/context.sh` as a script that pipes under `set -e`
  without `pipefail`. `lib/context.sh` sets nothing: it contains a comment that explains an
  errexit subtlety, and the detector matched the comment.

Each gate was right about what it wanted and wrong about what it was looking at. The failure
mode is specific and worth naming: **a detector that matches a literal will match the prose
that explains the literal, the fixture that demonstrates it, and another language that spells
it the same way.** A gate that documents its own subject matter is guaranteed to contain its
own subject matter.

# Required behaviour

**A verdict states its denominator.** A check that reports a count reports what the count is
out of, and a check that says "every one" names how many. `40 of 121 rule(s) carry a
validator` is a verdict; `40 doctrines — resolve for every one` is not, because the reader
cannot tell whether the 40 is the whole population.

**A verdict names what it did not examine.** Where a check looks at a subset, the remainder is
stated in the same breath, so that a reader cannot take the subset for the whole.

**Zero is a failure unless zero is declared expected.** A check that examined nothing says so
and fails; `project.empty-is-not-failure` governs the cases where an empty population is the
right answer, and those say it out loud.

**A detector distinguishes its subject.** A grep-based check excludes comments, single-quoted
program text of another language, heredoc bodies it authors as fixtures, and files of a
language it is not measuring. Where the exclusion cannot be exact, the check says which
direction it errs in.

**A ratchet over its own baseline is a bug, not a finding.** A baseline the tree is already
above cannot be reached by anyone and blocks every branch. When a ratchet fails, the first
question is whether its detector is right, and the answer is never to raise the baseline
because the number went up.

**A promise names a proof.** A rule declared `blocking` and a claim marked `guaranteed` each
name something a machine can follow to code that runs. `scripts/ci/enforcement-check` decides
this, and it is a ratchet for the same reason: the debt is real and predates the rule.

# Failure behaviour

`scripts/ci/enforcement-check` exits 10 on a blocking rule or a guaranteed claim whose proof
no machine can follow and which the baseline does not carry. `doctor` reports the rule
package's tally with both numbers beside each other, and reports the blocking rules with no
validator as a named finding rather than leaving them where only another surface can see them.

A bare count in a new check is refused at review. A gate whose detector cannot distinguish its
subject is a defect in the gate, and is repaired in the gate — never by recording its false
positives in a baseline, which converts a broken detector into permanent debt.

# Verification

`test/cases/230_verdict_states_its_subject.sh` proves both halves by mutation against fixture
trees rather than against this checkout: a rule set whose blocking rule names no proof is
refused, one that names a case by path or the way the runner takes it is accepted, a claim
whose test does not name it back is counted, and a baseline the tree is already above is
reported as unreachable rather than as a finding. `scripts/ci/enforcement-check --strict`
reports both populations with their denominators; `bin/majordomus doctor` prints the rule
tally.
