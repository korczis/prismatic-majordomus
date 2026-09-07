---
schema: adr/v1
id: adr-0021
kind: adr
title: The branch-to-worktree topology is derived from git identity and enforced everywhere
status: accepted
date: 2026-09-07
tags:
  - git
  - worktree
  - filesystem
  - agents
  - coordination
related:
  - rule:project.worktree-topology
  - rule:project.interfaces-are-projections
  - rule:project.rust-canonical-declaration
  - rule:project.no-machine-paths
  - file:apps/majordomus-cli/src/worktree/mod.rs
  - file:apps/majordomus-cli/src/worktree/path.rs
  - file:apps/majordomus-cli/src/worktree/migrate.rs
  - file:apps/majordomus-cli/src/capability/builtin/worktree.rs
  - file:docs/WORKTREES.md
provenance:
  origin: authored
---

# 21. The branch-to-worktree topology is derived from git identity and enforced everywhere

## Context

This repository is worked on by several sessions at once — people and agents, often four
or five concurrently — and each needs its own checkout of its own branch. Git's answer is
the linked worktree, and git takes the destination as an argument: it has no opinion about
where a worktree should live, so every caller has one.

On the day this decision was made the repository had fifty-three registered worktrees in
four shapes: thirty-two flat siblings named `prismatic-majordomus-<tag>` where the tag
(`cap`, `i1003`, `wtm`) said nothing about the branch, sixteen under session scratch
directories in the system temporary directory, one inside the primary checkout under
`.claude/worktrees/`, and one occupying `prismatic-majordomus-wt` — the one name a
container would want. Eleven of them held uncommitted work. No person and no agent could
answer "where is branch X checked out" without running `git worktree list` and reading, nor
"where does a new one go" without inventing an answer; and an agent told to "work on
feature/improve-cli" picked a plausible directory, worked for an hour, and left the work
where the next session would not look.

An earlier attempt at this decision (the `feature/worktree-management` branch) put a
`worktree:` section into the policy — a strategy, a suffix, a naming rule that flattened
`feature/foo` into `feature-foo` — and a second attempt would have added a registry
mapping branches to paths. Both add a thing that has to be kept in agreement with git.

## Decision

The topology is a **function of git identity and the branch name**, computed the same
way by every surface and stored nowhere:

```text
container(repository)        = parent(primary checkout) / basename(primary checkout) + "-wt"
worktree(repository, branch) = container(repository) / <branch name, one directory per component>
```

`~/dev/prismatic-majordomus` and `feature/providers/openai-streaming` therefore give
`~/dev/prismatic-majordomus-wt/feature/providers/openai-streaming`. The hierarchy is kept;
nothing is flattened, hashed or numbered. The primary checkout hosts the trunk, which is
discovered (the remote's HEAD, then `init.defaultBranch`, then the one conventional name,
then the primary checkout's branch) and never hardcoded. Every non-trunk branch that is
checked out has exactly one worktree, at that path.

**Git is the registry.** The identity comes from the common git directory and the main
worktree `git worktree list --porcelain` lists first, so the answer is the same from the
primary checkout and from four directories deep inside a linked worktree; the worktrees
come from that list; the branches, their upstreams and their worktrees from one
`for-each-ref`. The Rust executable interprets these into one typed
`RepositoryTopology` — every worktree with a standing (primary, canonical, misplaced,
detached, ephemeral, missing), every branch with or without a worktree, every diagnostic
under a stable code with a remedy — and every surface renders that value: the command
line (`majordomus worktree`), the four `worktree.*` capabilities (MCP tools and the
`majordomus://worktrees` resource, `/api/v1/worktrees*`, OpenAPI, Swagger UI), the Cockpit
page, the pre-commit guard and the generated reference. No file, section, table or list
records a branch's path, and adding a branch requires editing nothing.

**Legacy topology is input to a migration, never a second canonical form.** A worktree
somewhere else is a diagnostic (`worktree.path_mismatch`), and `majordomus worktree
migrate` brings it home with `git worktree move`, uncommitted and untracked work included,
under one repository-scoped lock, with a fingerprint — branch, HEAD, index, staged and
unstaged diffs, untracked files with their content, ignored entries by presence — taken
before and after each move and compared; a step is reported as moved only when the two are
equal. A worktree occupying the container path is moved out and then in; a move across
filesystems is made by copy, `git worktree repair`, verification against a manifest of
every entry of the tree, and only then removal of the original, and only when asked.
Nothing is ever reset, stashed, cleaned, checked out or deleted to satisfy the rule.

**Enforcement is layered and honest.** The doctrine and the project rule state it; the
provider bootstraps tell every agent; `majordomus worktree create` is the way a branch's
worktree comes into being; the pre-commit hook asks `majordomus worktree guard`, which
refuses a commit of a feature branch from anywhere but its canonical worktree and of the
primary checkout off the trunk; the topology is on every surface, so a wrong one is
visible; the crate's suite proves the derivation, the safety and the migration against
real git; and a CI gate holds the pieces together. Git hooks can be bypassed with
`--no-verify`, and a CI worker cannot see a contributor's local filesystem: this
decision does not claim otherwise. What it claims is that within Majordomus-controlled
workflows, wandering off the canonical path is refused, and outside them it is
painfully visible.

## Alternatives considered

**A `worktree:` policy section (strategy, suffix, naming).** Configuration for a fact
that has one right answer. Every knob is a way for two checkouts of one repository to
derive two containers, and the naming rule that flattened `feature/foo` to `feature-foo`
was lossy by design, which meant collisions to detect and a mapping to explain.

**A registry file mapping branches to paths.** The thing git already is, written down a
second time, guaranteed to disagree with it within a week. Rejected outright.

**`.worktrees/` inside the repository.** A checkout of branch B inside a checkout of
branch A: every build tool, watcher, linter and search walks into it, git has to be told to
ignore it, and an `rm -rf` of the repository takes B's uncommitted work with it.

**`~/.worktrees/<repo>/` or another global directory.** A place to look up rather than a
place to derive, that outlives the repositories it indexes and collides on basenames.

**Session-scoped temporary directories.** What was happening. Convenient for the agent
and invisible to everyone else; sixteen of the fifty-three were exactly this, and they are
now the `ephemeral` standing: reported, refused for commits, never migrated unasked, and
gone when the session that made them is.

**Flat sibling directories with a tag.** The other thing that was happening; thirty-two
of them, each requiring a person to remember what `i1011` was.

## Consequences

`-wt` is written once, as a constant in the Rust crate, and the derivation lives in one
module; everything else — every path in every message, page and document — follows from
it. The provider bootstraps, the documentation and the CI check name the suffix too, and
the check holds them to the constant.

The primary checkout must stay on the trunk. Committing to a feature branch from the
primary checkout is refused, which is a change of habit for anyone who used `git switch
-c` there; the remedy is one command.

The topology is answered from git on every call and never cached across calls: it changes
outside the process, and a cached answer would be an answer about a repository that no
longer exists. That costs a few subprocesses per call and a `git status` per worktree when
uncommitted work is asked for; a guard on the commit path runs one status, never one per
worktree.

Detached worktrees have no canonical path and are never moved; a scratch checkout of a
session (under the temporary directory or `.claude/worktrees/`) is reported and left to
the session that made it. Bare repositories are refused by name: the topology is defined
against a primary checkout.

This repository dogfooded the decision on the day it was made: thirty-two sibling
worktrees, eleven of them dirty, were migrated by the executable in under ten seconds with
every fingerprint equal; the exceptions were the sixteen ephemeral scratch checkouts, the
agent worktree inside the primary checkout, and one checkout outside every convention that
was left to the operator.
