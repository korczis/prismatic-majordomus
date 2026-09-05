---
schema: skill/v1
id: repo-review
version: 1
title: Repository review
description: An evidence-driven review of the actual state of the repository, or of one change to it, before meaningful engineering work is recommended or done.
status: active
tags: [review, evidence, invariants, drift]
inputs:
  - a checkout of the repository with git history available
  - the scope under review, or the whole repository when none is given
  - the effective context for every path in scope (majordomus context resolve <path>)
outputs:
  - findings, each with severity, evidence, location, the smallest correct fix and the evidence that would prove it
  - the list of what was checked, including the areas where nothing was found
---

# Purpose

Establish what is true about the repository before changing it. A worker that has not
looked starts "helping" by adding a layer nobody asked for; this procedure makes the
looking explicit, bounded and reportable. It reviews reality, not style: whether the code
does what the documents say, whether the guarantees are backed by tests, whether the
generated files match their sources, and whether the boundaries the repository declares
are the ones the code keeps.

# When to use

- before implementing an issue, to check that the repository is where the issue assumes;
- on a diff or a branch, before it is called done;
- on a subsystem or the whole repository, when the question is "what is actually here";
- on the generated surfaces (documents, projections, site data), when drift is suspected.

Not this skill: writing the fix, reviewing prose for tone, or estimating effort.

# Procedure

Work through the sections in order. Each one names what to read and what to look for.
Record every finding the moment it is observed, with the file and line it was observed
in; do not wait for the end to reconstruct evidence from memory.

## 1. Scope and context

1. Read the effective context for every path in scope: the root instruction file, the
   rules that apply, and the scoped documents from the root down to the directory
   (`majordomus context resolve <path>`, `majordomus rules list`). Note which rules are
   machine-enforced and which are enforced by a reviewer only; the second kind is where
   review earns its keep.
2. Establish the change set: the branch and worktree, the diff against its base, and the
   history of the files it touches. Read the surrounding implementation, not only the
   changed lines; a correct line in a wrong function is a wrong change.
3. State the scope in the report exactly as it was reviewed. Anything outside it is not
   covered, and the report says so.

## 2. Correctness

Look for behaviour that is wrong now, or that becomes wrong the first time the
environment is not the happy path:

- a violated invariant the repository states (a rule, a schema, a documented contract);
- an incomplete state transition, or a record that can be left half-written;
- an assumption the code makes that its inputs do not guarantee;
- persistence gaps: what is lost on restart, on a partial failure, on a second run;
- non-idempotent mutation: a command whose second run does something different;
- a hidden fallback, a swallowed error, a "warn and continue" where the contract says stop;
- two places that hold the same truth and can disagree.

## 3. Canonical and derived state

For every artifact in scope decide which it is: canonical, generated, cached, or
unrelated. Then check that the derived ones can be regenerated deterministically from
the canonical ones, that a stale one is detectable by a command that fails, and that
nothing writes from a projection back into its source. A generated file that carries no
mark saying so, and no check that fails when it is stale, is a second source of truth
waiting to happen. Run the repository's drift checks rather than reasoning about them.

## 4. Testing and evidence

For every behaviour the change claims, find the test that exercises it from the outside.
Look for:

- a claim with no test, or a test that asserts an implementation detail instead of the
  behaviour;
- a mock standing in for the very boundary the claim is about;
- a skipped or ignored test, or a check that reports success having examined nothing;
- documentation that promises more than the tests prove;
- a regression that the change could introduce and that no case would catch.

Run the tests that cover the scope. A test result you did not observe is not evidence.

## 5. Architecture and boundaries

Check the change against the boundaries the repository declares: provider-specific
content in provider-neutral places, a policy duplicated instead of referenced, a
dependency added without a stated need, an abstraction with one caller, infrastructure
that solves a problem nobody recorded, and data maintained by hand where it could be
derived. Name the declaration the change violates; a boundary that exists only in the
reviewer's taste is not a finding.

## 6. Performance

Only where performance is a stated contract or a hot path: read the benchmark evidence,
look for repeated parsing, scanning or generation inside a loop, and measure before
concluding. A performance finding without a measurement is speculation and is labelled
as such.

## 7. Documentation

Check that the documents describing the scope match its behaviour today, that generated
documents are in sync with their sources, that examples still run, that a guarantee's
status is honest, and that nothing on a roadmap is described as an existing capability.

# Output

The report has three parts, in this order.

**Scope reviewed.** What was read, which commands were run, and what was deliberately
not covered.

**Findings**, most severe first. Each one carries:

| field | content |
|---|---|
| severity | `FAIL` — must be fixed before the work is accepted; `WARN` — should be fixed, does not block; `INFO` — an observation with no required action |
| title | one line |
| evidence | what was observed, quoted or measured, with `path:line` |
| what is wrong | the fact, separated from the inference drawn from it |
| why it matters | the failure it causes, or the invariant it breaks |
| smallest fix | the least change that corrects it |
| proof | the test, check or command that would show the fix holds |

Mark every sentence as one of: observed fact, inference, recommendation, speculation.
Never promote an inference to a fact by leaving the label off.

**What was checked and found sound.** The areas of the procedure that produced no
finding, named, so that an empty findings list is distinguishable from a review that did
not look. If there are no substantive findings, say so in those words.

Never invent a finding to make the review look useful, and never omit one to make the
change look ready.
