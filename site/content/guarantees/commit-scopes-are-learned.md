+++
title = "The scope vocabulary a commit is judged against is learned from this repository's own history rather than kept in a table, so a subsystem committed today is in the vocabulary today and a scope outside it is a warning rather than a refusal"
description = "The usual way to answer *\"what scope does this change belong to?\"* is a table of path"
weight = 186
[extra]
claim_id = "commit-scopes-are-learned"
status = "guaranteed"
source = "docs/claims/commit-scopes-are-learned.md"
+++
{% raw %}

## What it means

The usual way to answer *"what scope does this change belong to?"* is a table of path
patterns mapping to scope names. It is correct on the day it is written. Then a directory is
added, or renamed, or a subsystem splits in two, and the table names a scope the history
stopped using months ago — with nothing to notice, because a table has no way to be wrong.

The claim is that this repository keeps no such table. The vocabulary a commit is judged
against is observed from the repository's own log, so a subsystem committed today is in the
vocabulary today and one nobody has touched for a year sinks on its own.

## How it works

`commit.scopes` reads at most 1500 commits with one `git log --no-merges --name-only`, parses
each subject with the same grammar the changelog uses, and counts two things: how often each
scope is used, and how often each scope is used *about* each directory prefix four segments
deep. Every commit in the history is a worked example of which scope a set of paths belongs
to, decided by whoever made the change and kept by whoever reviewed it.

Four segments is measured, not chosen. This repository's source lives at
`apps/majordomus-cli/src/<subsystem>/`; three segments stop one level above the subsystem,
where a dozen scopes all have commits and no path can be told from another. The first version
used three and reported every source change as belonging to no scope at all.

`suggest(paths)` votes each path at the deepest prefix anything is associated with — so a
change inside one subsystem is not outvoted by the repository's busiest directory two levels
up — and reports three things: the scope, the number of prior commits that associate that
scope with that directory, and whether another scope was equally associated. A tie is reported
as `ambiguous` rather than broken silently. Paths nothing in the history associates with any
scope yield `None`, which is the honest answer for the first commit of a new subsystem and
better than the most popular scope in the repository, which is what a tie-break to frequency
would produce.

A scope outside the vocabulary is a **warning**, never a refusal. Inference must not refuse
the first commit of a subsystem it has never seen — this very subsystem's first commit carried
`commit.unknown_scope`, which is the design working rather than failing.

## How to see it

```bash
majordomus commit scopes                     # every scope, how often, about which directories
majordomus commit scopes --format json       # the same, with the sample size
echo "feat(nonesuch): a thing" | majordomus commit validate
                                             # exit 0, warning commit.unknown_scope
bash test/run.sh 274_commit_policy           # the case proves the vocabulary follows the history
```

The case is the interesting half: it builds a repository, commits about `alpha` and `beta`,
asserts that `gamma` is unknown and warned about, commits about `gamma`, and asserts that
`gamma` is then known. Nothing was configured between the two assertions.

## What it does not cover

It is not free. Reading names out of the log costs about a second on this history, and it is
deliberately on no hot path: nothing about entering the repository, completing a word or
answering `--help` reads it. It is read when a commit is being planned or judged, which is a
moment that already involves waiting for git.

It says nothing about what a scope *means*. It observes which word was used about which
directory, not whether that word was a good choice — a repository that has consistently used
a bad scope name will be told, consistently, that the bad name is the one it uses.

## Why it exists

The reference procedure this subsystem replaced carried a literal Markdown table of path
patterns to scopes, maintained by hand. Every row of it was a fact written in two places, one
of which changes — the shape `DYNAMICITY.md` names, and the one this repository spends its
time removing. The repository already knew the answer; it was in the log.
{% endraw %}
