---
schema: adr/v1
id: adr-0020
kind: adr
title: Linked worktrees live in a sibling container derived from the primary checkout
status: accepted
date: 2026-09-06
tags:
  - git
  - worktree
  - filesystem
  - agents
provenance:
  origin: authored
---

# 20. Linked worktrees live in a sibling container derived from the primary checkout

## Context

This repository is worked on by several sessions at once — people and agents, often four or
five concurrently — and each of them needs its own checkout of its own branch. Git's answer
is the linked worktree, and git takes the destination as an argument: it has no opinion
about where a worktree should live, which means every caller has one.

The result was measurable rather than theoretical. At the time of writing, this repository
had thirty-seven linked worktrees in four different shapes: twenty-six flat siblings named
`prismatic-majordomus-<tag>`, eleven under a session-scoped path in the system temporary
directory, and — the case that made the problem concrete — one worktree occupying
`prismatic-majordomus-wt`, the very name a container would want.

Nothing about that was anybody's mistake. Each one was a reasonable local decision. The cost
is that the set of them cannot be described, only enumerated: no person and no agent could
answer "where are this repository's worktrees" without running `git worktree list` and
reading, and none could answer "where does this new one go" without inventing an answer. A
question that has to be answered afresh every time gets answered differently every time, and
what accumulates is directories of work nobody remembers creating and nobody dares delete.

The specific failure that recurs with agents is worse than untidiness. An agent asked to
"work on issue 184" has to pick a path. It picks a plausible one — the current directory, a
temporary directory, a subdirectory of the repository — works for an hour, and the work ends
up somewhere the next session will not look. The decision was never the agent's to make; it
was pushed onto the agent because nothing else had made it.

## Decision

Every linked worktree of a repository lives directly under a container that is the
repository's sibling:

```text
<parent>/<repo>          the primary checkout, which never moves
<parent>/<repo>-wt/      the container
<parent>/<repo>-wt/*     every linked worktree
```

The container is **derived, never chosen**: it is `parent(primary) + basename(primary) +
suffix`, where `suffix` is a single value in `worktree.root` of the canonical policy. The
primary checkout is identified from the repository's shared git metadata — the common git
directory and the main work tree `git worktree list --porcelain` reports first — and never
from the current directory. That last point is the load-bearing one: it is what makes the
answer identical whether the command runs in the primary checkout or four directories deep
inside a linked worktree, and it is what prevents `<repo>-wt/issue-123-wt`, a container per
worktree, nesting for ever.

`majordomus worktree` owns the lifecycle: `create` derives the destination and refuses to
take one, `list` and `status` report the topology against the policy, `path` gives a shell
something to `cd` into, `remove`, `migrate` and `prune` change it under one
repository-scoped lock. `majordomus doctor` reports every worktree that is somewhere else
and repairs nothing.

## Alternatives considered

**`.worktrees/` inside the repository.** The obvious first idea, and it puts checkouts of
other branches inside a checkout. Git must be told to ignore it, and the ignore rule is not
enough: every build tool, test runner, linter, file watcher and search that walks the tree
finds it, and each has to be told separately. A worktree of branch B inside a checkout of
branch A also means a naive `rm -rf` of the repository takes B's uncommitted work with it.

**`~/.worktrees/<repo>/` or another global registry.** Solves discovery by centralising it,
and introduces a directory whose contents outlive the repositories they belong to. A
repository moved or deleted leaves its worktrees behind with nothing pointing at them, and
two repositories with the same basename collide. It also puts the answer somewhere that has
to be looked up rather than derived, which is the property being removed.

**Arbitrary developer paths, documented as a convention.** What was already happening. A
convention with no derivation and no check is a suggestion, and the thirty-seven worktrees
in four shapes are what a suggestion produces.

**Per-agent temporary directories.** Convenient for the agent and hostile to everyone else:
work disappears at reboot, is invisible to the next session, and cannot be found by anyone
who did not start that session. Eleven of the thirty-seven were exactly this.

**The sibling container.** Outside the source tree, so nothing is committed, no ignore rule
is needed and no tool walks into it. Beside the checkout, so it is found by looking next to
the thing you already have, with no registry and no lookup. Named after the repository, so
two repositories never collide. Holding exactly one repository's worktrees, so it is a
bounded cleanup boundary. Derivable from one suffix, so it is the same answer for every
caller from every directory — which is the property an agent needs and the one none of the
alternatives has.

## Consequences

The `-wt` convention exists once, in `worktree.root.suffix` of `.ai/repo/policy.yaml`.
Everything else derives: the commands, the doctor check, the provider instruction files
through the `{{WORKTREE_SUFFIX}}` token, and the tests, which change the policy rather than
the code to prove the derivation is real. Changing the suffix moves every container and
fails `majordomus generate --check` until the provider files are regenerated.

The doctrine is a machine-local check about this machine's directories, so it is
deliberately not a CI gate: a CI runner's checkout is a normal checkout and is never
expected to sit under a container. What CI validates is the repository-level half — the
policy against its schema, the provider projections, the behavioural cases.

This repository does not comply with its own decision on the day the decision is made.
Thirty-seven linked worktrees are outside the container and one of them occupies the
container's own path, so `majordomus doctor` fails here until they are migrated. That is the
intended behaviour of a rule that was adopted after the situation it describes, and the
migration is an explicit operation a person runs, not something a tool does on their behalf.
The worktree at the container's path has to be moved first, by hand, because a directory
cannot be both the container and a checkout.

Bare repositories are out of scope and refused by name: worktree management is defined
against a primary checkout, and a bare repository has none.
