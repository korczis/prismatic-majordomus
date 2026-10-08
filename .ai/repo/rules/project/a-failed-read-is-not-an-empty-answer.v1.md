---
id: project.a-failed-read-is-not-an-empty-answer
version: 1
kind: rule
title: A source that did not answer is not a source with nothing in it
description: A script that asks a remote for a listing and keeps the answer says so when the asking fails and stops; it never leaves an empty file or an empty set for the next step to read as the remote's real state.
statement: When a read of something outside the process fails, the script reports that it could not read and exits unusable (12), having written nothing; the answer of a remote is never written to a file with the failure of asking discarded, and a decision is never made from an answer that was not had.
status: active
class: blocking
depends_on: [project.github-projection-gated@1, project.no-claim-without-test@1]
tags: [projection, github, shell, failure, enforcement]

x-majordomus:
  tests: [test/cases/894_github_sync_applies_only_the_trunk.sh]
---

# Rationale

`scripts/github-sync` lists the milestones and the issues of the repository before it
compares them with the plan. Both listings ended in `2>/dev/null || true`.

So a listing GitHub refused, cut short or rate-limited left an empty file, and everything
after it read that file as the remote: no milestone exists, no issue exists. `--check`
reported every canonical record `missing` and exited 11, the exit of ordinary drift; the gate
above it accepts 11 as "the remote was read" and would have reported a broken ratchet where
the truth was that nothing had been read. `--apply`, run in that minute, would have created
every issue a second time. The line that lists the labels, ten lines further down, already
did it properly and said why in a comment. Two listings beside it did not.

The failure is the reverse of the one `project.github-projection-gated` names. That rule
says a gate that cannot reach the remote must not pass. This one says it must not fail *as
if it had reached it* either: an answer that was not had is neither agreement nor drift.

A gate that reports a finding when there is nothing to find is the same confusion met from
the other side — the empty set read as a failure rather than a failure read as the empty
set — and the two are worth reading together.

# Required behaviour

- A script that reads a remote and keeps the answer checks that the read succeeded. When it
  did not, it says which read failed, with the command that reproduces it, writes nothing
  further and exits 12.
- The answer of `gh` or `curl` is not written to a file with the failure of the command
  discarded (`> file 2>/dev/null || true`, `|| :`).
- An answer kept in a variable is tested where it is set, and its emptiness is carried as
  "unknown", by name, to whatever prints or decides — never as zero, none or clean.
- A run that could not read makes no write: no issue is created, edited, closed or reopened
  from a listing that was not had, and a lock taken for the run is released.

# Failure behaviour

`scripts/ci/shell-lint` reads every tracked shell script with
`scripts/lib/swallowed-listings.awk` and fails, naming the file and the line, on a command
that asks `gh` or `curl`, writes the answer to a file and ends in `|| true` or `|| :`. The
scanner is run against `scripts/lib/swallowed-listings.fixture` first, whose answer is
written in it, so a scanner that stopped finding the shape cannot pass for a clean tree.
There is no baseline: the tree holds none.

`test/cases/894_github_sync_applies_only_the_trunk.sh` drives the adapter with a `gh` that
refuses each listing in turn, for `--check` and for `--apply`: the exit is 12, the message
names the listing, no `DRIFT` line is printed, no write reaches `gh`, and the lock is gone.
The same case puts the discarded failures back into a copy of the adapter and requires the
scanner to name them.

What it does not catch, stated rather than implied: a probe whose answer goes into a
variable (`x="$(gh ... || true)"`) is not reported, because whether the empty value is then
read as "unknown" cannot be told from the line; nine such probes exist and each is review's
to read. A listing that succeeds and is merely incomplete — a page GitHub dropped without an
error — is not detected by anything here.

A sibling shape is not reported either: a tool that could not start, its exit status
returned as a verdict about its input — a JSON reader behind a version shim with no version
set, read as "this file does not parse". The scanner looks for a remote's answer kept in a
file, not for an exit status used as a judgement, and the fix for that shape is to tell
"could not run" from "ran and refused" where the tool is called.

# Verification

```sh
scripts/ci/shell-lint
bash test/run.sh 894_github_sync_applies_only_the_trunk
```
