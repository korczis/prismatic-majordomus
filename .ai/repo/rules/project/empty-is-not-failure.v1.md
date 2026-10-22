---
id: project.empty-is-not-failure
version: 1
kind: rule
title: An empty result and a failed command must not look alike
description: A command that could not answer is reported as a command that could not answer, never as a command that answered nothing; a caller that cannot tell the two apart acts on a confident wrong answer.
statement: Distinguish "there is nothing" from "I could not look", at every boundary where one program reads another's output, and make the second one say so.
status: active
class: blocking
depends_on: [project.finding-carries-reproduce@1]
tags: [evidence, shell, diagnostics]

x-majordomus:
  reviewed_because: the rule is about a class of mistake, not a syntax; the one instance with a remedy is in scripts/generate-site-data and a check that recognised the shape would have to understand what each filter means
---

# Rationale

Three sessions met this in one evening, in three languages, and the third one is why the
rule is blocking rather than advisory.

`grep '^FAIL'` matching nothing exits non-zero and prints nothing, so a build step that died
before it wrote a line left a headline with silence underneath it. An hour went into finding
the cause, which was a full disk.

`kill` followed by `wait` on the process it killed returns the signal status, so a test case
under `bash -eu` ended with no assertion message, no diagnostic and a bare FAIL — the case
had not failed, the cleanup had.

And a `find -newermt` expression that this machine's `bfs` rejects outright printed its
rejection to stderr and nothing to stdout, and the caller read the empty stdout as the
answer: "no worktree has been edited in six hours." That is not a hidden reason. That is a
confident wrong answer, and it was acted on.

The shape is the same each time: a program asked another program a question, the second could
not answer, and the first could not tell that from an answer of "none". A gate that passes
over an empty set is the same fault one layer up — this repository shipped a schema check
that examined 0 of 40 files and reported OK, and the test that owned it wrote its fixture at
the one depth the broken glob could reach.

# Required behaviour

1. **Check the status, not the output.** A pipeline whose last command is `grep`, `find`,
   `jq -e` or any matcher must distinguish exit 1 (matched nothing) from exit 2 and above
   (could not look). Under `set -e` and `set -o pipefail`, guard deliberately with `|| true`
   and then read the status that was saved, rather than letting the shell decide.
2. **Say which it was.** A step that could not run names the command, the exit code, and what
   it printed to stderr. "Nothing found" and "could not search" are different sentences and a
   reader must never have to guess which one they are looking at.
3. **A count of zero is a claim and needs the same evidence as any other.** A check that
   reports success over an empty corpus asserts that the corpus is empty; assert that against
   something measured from disk, not against the absence of output.
4. **Cleanup does not fail a case.** `kill`, `wait` and teardown run outside the assertions
   they follow, and their statuses are not the case's verdict.

# Failure behaviour

Decided by review and by the gates that already read one another's output. Where a check can
be made to prove its own corpus is non-empty it should, because that is the assertion which
would have caught the schema case and the only one that would.

# Verification

`scripts/generate-site-data` is the instance with a remedy behind it: both blocks that read
another program's output now name the exit code and fall back to the captured stderr when
their filter renders nothing, so a step that died before writing a `FAIL` line can no longer
leave a headline over silence. The other two instances are recorded in the Rationale as the
evidence that this is a class rather than an incident; neither has a gate, and this rule
exists so that the next one is recognised rather than diagnosed from scratch.
