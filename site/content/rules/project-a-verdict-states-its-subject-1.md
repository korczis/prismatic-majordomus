+++
title = "A verdict states what it is a verdict about"
description = "A verdict states what it is a verdict about"
weight = 59
[extra]
kind = "rule"
slug = "project-a-verdict-states-its-subject-1"
identity = "project.a-verdict-states-its-subject@1"
status = "active"
source = ".ai/repo/rules/project/a-verdict-states-its-subject.v1.md"
+++
{% raw %}

## Rationale

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

## Required behaviour

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
name something a machine can follow to code that runs. Two gates decide this, one per
population, and each is a ratchet for the same reason — the debt is real and predates the
rule: `scripts/ci/rule-proof-check` for rules, `scripts/ci/claim-proof-check` for claims.

**A gate says which population it is not about.** Where two checks measure neighbouring
populations, each names the other's on every run rather than leaving the reader to infer it
from a list of identifiers. The two gates above were one script carrying both halves, and its
failure report printed rule ids and claim ids in one list under a remedy paragraph that led
with blocking rules; the list of unproven *claims* was read as a list of unproven *rules*, and
the two gates were reported as contradicting each other while they agreed. A verdict that
states its denominator and not its population is still not a verdict about anything nameable.

**One property has one reader.** A second inventory of a property another gate already owns is
not a harmless redundancy — it drifts, and the weaker of the two becomes wrong. The removed
half accepted any `x-majordomus` block as proof, so when ADR 0048 put `reviewed_because:`
inside that block it read four rules that declare a *person* enforces them as machine-followed
enforcement, and reported the exemptions as promises that had gained a proof. The repair is to
delete the duplicate reader, never to teach the two to agree.

## Failure behaviour

`scripts/ci/rule-proof-check` exits 10 on a blocking rule whose proof no machine can follow
and which `.ai/repo/rule-proof-baseline.txt` does not carry, and `scripts/ci/claim-proof-check`
exits 10 on a guaranteed claim whose test does not name it back and which
`.ai/repo/claim-proof-baseline.txt` does not carry. Each states both denominators and names the
population it did not measure. `doctor` reports the rule package's tally with both numbers
beside each other, and reports the blocking rules with no validator as a named finding rather
than leaving them where only another surface can see them.

A bare count in a new check is refused at review. A gate whose detector cannot distinguish its
subject is a defect in the gate, and is repaired in the gate — never by recording its false
positives in a baseline, which converts a broken detector into permanent debt.

## Verification

`test/cases/230_verdict_states_its_subject.sh` proves this by mutation against fixture trees
rather than against this checkout: a blocking rule that names no proof is refused and one that
names a validator or a case is accepted; a rule carrying only `reviewed_because:` is reported
apart and never as proof; a guaranteed claim whose test does not name it back is counted; a
rule whose body quotes `class: blocking` while its front matter says otherwise is not counted;
and a population that is empty where the subject exists exits unusable rather than clean. The
section that pins the boundary drives one tree holding both an unproven rule and an unproven
claim through both gates, and requires each to report its own finding, to report neither the
other's, and to say in words which population it did not measure.
`scripts/ci/rule-proof-check --strict` and `scripts/ci/claim-proof-check --strict` each report
their population with its denominator; `bin/majordomus doctor` prints the rule tally.
{% endraw %}
