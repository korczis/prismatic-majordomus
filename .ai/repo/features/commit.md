---
schema: feature/v1
id: commit
kind: feature
title: A commit message is a value this repository judges, not a convention it hopes for
short_title: Commits
headline: The commit message has one grammar, one policy and one verdict; the scope it is written under is learned from the repository's own history rather than kept in a table, and a message that does not satisfy the policy is refused while it is still being written.
summary: The same parse the changelog reads the history with decides whether a new message passes. The policy is data in the repository's own policy file, its subject width measured from the history rather than taken from the convention. The scope vocabulary is observed from at most 1500 commits of the log, so a subsystem committed today is in it today and a scope nobody uses sinks on its own. A plan over the working tree carries a fingerprint of repository, worktree, HEAD and change set, so a plan acted on after the tree moved is refused rather than applied - and a change set holding generated files is one commit, because only the last of several could carry current derived data.
status: stable
weight: 205
featured: true
areas: [governance, work-tracking]
modules: [commit]
rules: [project.conventional-commits, project.derived-files-regenerated, project.interfaces-are-projections]
docs: [docs/COMMIT.md]
adrs: [adr-0054]
claims: [commit-message-is-judged, commit-scopes-are-learned, commit-plan-is-refusable]
related: [doctrine, declare-once, provenance]
tags: [git, commit, governance]
---

## What it does

`majordomus commit validate` judges one message against the policy declared in
`.ai/repo/policy.yaml` and reports typed findings — the grammar, the subject width, a scope
the history has not used, a record id the layer does not hold, a `fix` with no test among
its files, a breaking change that explains nothing. `.githooks/commit-msg` asks it with the
staged paths, which is the one moment the message and the files both exist, and git aborts
the commit when a finding is an error. `scripts/ci/commit-policy` asks the same judge of the
history through `majordomus commit history`, which reads 1788 commits in about a second.

`majordomus commit scopes` answers which scopes this repository writes about which
directories, learned from the log. `majordomus commit plan` says what the working tree would
commit, divided into the commits the history's own scoping supports, each with the evidence
for it and the whole under a fingerprint that makes a stale plan refusable.

All four are read-only capability declarations, so the command line, MCP, HTTP, OpenAPI, the
Cockpit and the generated reference derive from one place and none of them keeps a list.

## What it does not do

It does not make the commit. `git commit` is a repository mutation and the exposure policy
stops every machine surface at local mutation: an agent may ask what a commit would be and
whether a message passes, and a person commits. The subsystem is shaped to that — the
planner and the judge are pure functions of a tree and a message, so they work offline,
cannot half-succeed, and every answer can be recomputed and checked.

It does not judge whether the subject is a good sentence, whether the change is atomic, or
whether the work was worth doing. A reviewer decides those. It does not propose a subject
either: what a change did is the one thing no evidence in the tree can state, and a planner
that wrote a confident sentence about it would be writing fiction into the history.
