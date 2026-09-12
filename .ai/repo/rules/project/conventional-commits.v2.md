---
id: project.conventional-commits
version: 2
kind: rule
title: A commit message is a value this repository judges, not a convention it hopes for
description: The commit message has one grammar, one policy and one judge in the executable; the message a person writes is refused at commit-msg when it does not satisfy the policy, the history is measured against a recorded baseline that may not grow, and the scope vocabulary is learned from the history rather than maintained in a table.
statement: A commit message is parsed by one grammar, judged against the policy declared in .ai/repo/policy.yaml, and refused at the moment it is written; the scope vocabulary is derived from this repository's own history; a subject git composed is exempt rather than wrong; and the history's existing debt is recorded and may not grow.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.blocking-checks-cheap@1, project.no-claim-without-test@1]
tags: [git, commit, governance]

x-majordomus:
  tests: [test/cases/274_commit_policy.sh, scripts/ci/commit-policy]
---

# Rationale

Version 1 of this rule said what a commit message should look like and ended with *"No
command decides this rule; a reviewer does."* That sentence is the failure the rest of this
repository's governance exists to prevent: a stated invariant with no verdict behind it. It
was also, measurably, not working. Over 1161 non-merge commits the median subject is 73
characters, 48 exceed even the width this project declares for a line of anything, and 38
carry a type word no vocabulary contained — none of which anybody noticed, because nothing
was looking.

The parser was already here. `release::commits` has read conventional commits out of the log
to build the changelog since the changelog existed. What was missing was not a grammar; it
was a judge, and a moment for it to speak.

The other half of the problem was outside the repository entirely. The *procedure* for
writing a commit — derive a scope from the path, link the work, do not invent an issue
number, a fix needs a test — lived in prose that an AI client read and carried out by hand,
one file per client, with a path-to-scope table maintained by a person. A workflow whose
semantics live in a prompt is unversioned, unenforceable, untestable, and true for exactly
one tool. The same table is derivable: every commit in the history is a worked example of
which scope a set of paths belongs to, decided by whoever made the change and kept by
whoever reviewed it.

# Required behaviour

**One grammar.** `commit::header` parses a subject; the changelog and the judge both read
that parse. The changelog carries what it cannot classify rather than dropping it; the judge
reports it. Two verdicts, one parse — there is no second regular expression in a hook, a
gate or a script.

**One policy, declared as data.** `commit:` in `.ai/repo/policy.yaml` carries the subject
width and three levels; the schema `majordomus.policy/v1` owns its shape and
`share/allow/policy.txt` is generated from that schema. The type vocabulary is *not* in the
policy: it is `ChangeKind`, which the changelog already renders from, and a second copy of
those words is the duplication this rule exists to remove.

**Judged where its evidence is.** The `commit-msg` hook asks `commit.validate` with the
staged paths, which is the one moment the message and the files both exist — so a `fix` with
no test among its files can be noticed while adding one is still cheap. The hook carries no
logic; it asks.

**The vocabulary is observed, not declared.** `commit.scopes` learns which scopes this
repository uses, how often, and about which directories, from at most 1500 commits of its
own log. A scope outside it is a warning, never a refusal: inference must not refuse the
first commit of a subsystem it has never seen.

**Nothing is invented.** A record id in a message — `I1305`, `M000` — is resolved against
what the layer holds, and one that resolves to nothing is an error. This is the converse of
the resolution the changelog already does, and the two read one scan, so they cannot come to
disagree about what an id looks like.

**Git's own subjects are exempt, not wrong.** `Merge …`, `Revert "…"`, `fixup!` and
`squash!` are composed by git; a validator that refused them would be refusing git. A
`merge:` subject a person wrote is *not* exempt — this repository writes those deliberately,
so `merge` is a declared type word and those commits are held to the same width and the same
references as every other.

**Existing debt is recorded and may not grow.** 54 commits in this history do not satisfy
the policy. Every one of them is published, so rewriting them is not on the table; the count
lives in `.ai/repo/commit-policy-baseline.txt` and the gate fails when it rises.

# Failure behaviour

The `commit-msg` hook (`.githooks/commit-msg`, registered under `enforcement` in
`.ai/repo/policy.yaml` as `commit-policy-on-message` and reconciled by `majordomus doctor`)
exits 10 and git aborts the commit, printing every finding with its code.

The gate `commit-policy` (`scripts/ci/commit-policy`) fails, exit 10, when a commit this
branch adds over the trunk does not satisfy the policy — outright, with no baseline, because
the hook refused that message while it was being written — or when the history's recorded
debt grows. A checkout with no trunk to compare against reports that it measured nothing
rather than reporting a pass.

`test/cases/274_commit_policy.sh` is the behavioural half, in disposable repositories: that a
good message passes and a bad one is refused at exit 10, that a merge subject is exempt, that
an invented record id is refused and a real one is not, that a fix with no test is reported
and does not refuse, that the vocabulary comes from the history and follows it when the
history changes, that a plan is refused once HEAD or the index moves under it, that two
worktrees of one repository do not share a plan, and that the gate itself can fail.

# Verification

```sh
scripts/ci/commit-policy                      # the gate
bash test/run.sh 274_commit_policy            # the behavioural case
majordomus commit validate <<'EOF'            # one message
feat(commit): a subject
EOF
majordomus commit history origin/master..HEAD # this branch's commits
majordomus commit scopes                      # the vocabulary, and what it was learned from
majordomus commit plan                        # what the working tree would commit, and why
```

`apps/majordomus-cli/src/commit/` carries the unit half: that the grammar round-trips, that
an unparsed header is one finding and not five, that a tie in the vocabulary is reported
rather than broken, that grouping is a function of the tree and not of the order git reported
it in, and that a fingerprint names what moved.
