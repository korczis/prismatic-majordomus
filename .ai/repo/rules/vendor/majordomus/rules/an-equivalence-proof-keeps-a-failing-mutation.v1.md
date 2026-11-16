---
id: majordomus.an-equivalence-proof-keeps-a-failing-mutation
version: 1
kind: rule
title: An equivalence proof keeps a recorded mutation that fails it
description: A test that claims new code behaves exactly as old code did carries a permanent control, a planted change that must fail it.
statement: Keep, beside every test that claims "identical to before" or "unchanged by this refactor", a permanent control in which a planted change to the code under test makes the test fail, and in which a copy of the test that cannot see that change fails the control.
status: active
class: advisory
depends_on: [majordomus.verify-outcomes@1]
tags: [testing, evidence]
---

# Rationale

An equivalence test compares two things and passes when they agree. It passes just as well
when both sides are the same thing, when both are stubs, or when the comparison reads
nothing that the change could move. A test like that cannot fail, and a green run of it is
activity rather than an outcome (`majordomus.verify-outcomes@1`): it proves the comparison
ran, not that the code under it is unchanged.

Observed in algoglyph, a public repository: a test claimed that every graph plate drew the
same calls through the new engine as through the old one. It loaded both engines through a
mocked module cache, so both sides drew the mock, and the comparison was always equal. It
passed for weeks while a planted change sat in the new engine. Nothing in the test's output
showed this. Only a mutation it should have caught showed that it could not fail.

# Required behaviour

Any test whose claim is equivalence ("draws the same calls as before", "the output is
byte-identical to the old implementation", "this refactor changes nothing") keeps a
permanent control beside it, committed and run with it:

- **A planted change fails the test.** The control applies one small, named change to the
  code under test (a constant, a branch, an argument), runs the equivalence test against
  it, and requires it to fail. The control runs on every run of the test, not once when
  the test was written. A mutation that was tried once and then deleted proves only that
  the test could fail on that day.
- **A vacuous copy fails the control.** The control is itself shown to have teeth: a copy
  of the equivalence test that cannot see the change fails the control. In such a copy
  both sides come from one source, or the code under test is stubbed out, or nothing is
  compared. That is the shape the observed failure had, and it is how the control proves
  it would have caught it.
- **Both sides are real.** The two sides of the comparison are loaded through separate
  paths that each reach the actual code: no shared module cache, mock or fixture stands in
  for both. Where one side must be a recorded reference, the control names the record and
  the revision it was recorded at.
- **The control says what it planted.** Its name or message states the planted change and
  the line of the equivalence test that must turn red, so a failure of the control is
  readable without reading the test.

A change that weakens the control or deletes it is a change to the equivalence claim and
is reviewed as one.

# Failure behaviour

Nothing decides this rule directly: it has no validator, so a violation is reported by
nobody. It is normative text. A worker or reviewer applies it when they write or review an
equivalence test, and treats a missing or toothless control as a test that cannot fail.

# Verification

Read the equivalence test with its control. The control applies a named change to the code
under test and requires the test to fail. A vacuous copy of the test, run through the
control, fails it. Both run on every run of the suite. A repository may add a project rule
with a validator that decides part of this mechanically, for example that every test tagged
as an equivalence proof has a control file beside it.
