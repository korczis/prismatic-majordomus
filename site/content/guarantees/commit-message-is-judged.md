+++
title = "A commit message that does not satisfy the repository's declared commit policy is refused at the moment it is written, and the history's existing debt is recorded and may not grow"
description = "The rule project.conventional-commits used to state what a commit must look like and end"
weight = 185
[extra]
claim_id = "commit-message-is-judged"
status = "guaranteed"
source = "docs/claims/commit-message-is-judged.md"
+++
{% raw %}

## What it means

The rule `project.conventional-commits` used to state what a commit must look like and end
with the sentence *"No command decides this rule; a reviewer does."* That is a stated
invariant with nothing behind it, and the measurement taken the day it was replaced says what
that cost: over 1161 non-merge commits, 48 subjects exceeded even the width this project
declares for a line of anything and 38 carried a type word no vocabulary contained. Nobody
had noticed, because nothing was looking.

The claim is that a message is now judged, by a command, at the moment somebody writes it —
and that the judgement is the same one the gate applies to the history and the same one an
agent is handed over MCP.

## How it works

One parse. `commit::CommitHeader::parse` reads a subject into a type word, a scope, a
breaking mark and a subject, and never fails. `release::commits::parse` — the changelog's
reader, which has existed since the changelog existed — delegates to it. The two callers
disagree only about what to *do* with a header that is not conventional: the changelog
carries it, because a changelog that drops what it cannot classify lies by omission; the
judge reports it.

One policy, as data. `commit:` in `.ai/repo/policy.yaml` carries the subject width and four
levels; `majordomus.policy/v1` owns its shape and `share/allow/policy.txt` is generated from
that schema, so adding a key is one edit and a regeneration rather than an edit in three
places. The type vocabulary is deliberately *not* there: it is `ChangeKind`, which the
changelog already renders headings from.

One judge, three callers. `commit.validate` takes a message, the vocabulary, the paths when
they are known, and the record ids the layer holds, and returns typed findings over the
repository's own `Diagnostic`. `.githooks/commit-msg` asks it with `git diff --cached
--name-only`, which is the one moment the message and the files both exist — so
`commit.fix_without_test` can be asked there and is deliberately not asked of the history,
where the evidence is absent and the judgement would be a guess. `scripts/ci/commit-policy`
asks `commit.history`, which judges 1788 commits in 992 ms — one process for the range
rather than one per commit, which is what lets it be a gate rather than a nightly job.

The hook is declared in the policy's `enforcement` list as `commit-policy-on-message`, wired
by `git-hook:commit-msg`, so `majordomus doctor` holds it the way it holds `doctor-on-commit`
and `derived-current`: the path must resolve, be executable, be invoked by the named artifact,
and not have its exit code swallowed. The rule is enforced twice over — the hook refuses the
message, and the doctrine refuses the repository that unwires the hook.

## How to see it

```bash
echo "update stuff" | majordomus commit validate          # exit 10: commit.not_conventional
echo "fix(plan): a thing under I9999" | majordomus commit validate
                                                          # exit 10: commit.unresolved_reference
echo "Merge pull request #1 from a/b" | majordomus commit validate
                                                          # exit 0: exempt, git composed it
majordomus commit history origin/master..HEAD             # this branch's commits
scripts/ci/commit-policy                                  # the gate
majordomus doctor | grep commit-policy-on-message         # OK wiring … via .githooks/commit-msg
bash test/run.sh 274_commit_policy                        # the behavioural case
```

## What it does not cover

It does not judge whether the subject is a good sentence, whether the change is atomic, or
whether the work was worth doing. A reviewer decides those, and a validator that pretended to
is a validator nobody trusts.

It is a local hook, so it binds whoever has `git config core.hooksPath .githooks`. A commit
made without it, or a merge made through the forge, does not pass through it; the
`commit-policy` gate in CI is the backstop, and it holds a branch's own commits to the policy
outright, with no baseline, precisely because the hook should already have refused them.

54 commits of this history do not satisfy the policy. Every one is published, so rewriting
them is not on the table; the count lives in `.ai/repo/commit-policy-baseline.txt` and the
gate fails when it rises rather than pretending the history is clean.

## Why it exists

The procedure for writing a commit — derive a scope from the path, link the work, do not
invent an issue number, a fix needs a test — used to live in prose that an AI client read and
carried out by hand, one file per client, with a path-to-scope table maintained by a person.
A workflow whose semantics live in a prompt is unversioned, unenforceable, untestable, and
true for exactly one tool. The parser that could have decided it was in this repository the
whole time, reading the history for the changelog. What was missing was not a grammar. It was
a verdict, and a moment for it to speak.
{% endraw %}
