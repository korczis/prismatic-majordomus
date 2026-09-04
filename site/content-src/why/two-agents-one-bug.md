+++
title = "Two agents fixing the same bug in two branches"
description = "Why isolation alone defers the collision, and how declared scope and overlap reports prevent duplicated work."
weight = 2
[extra]
hook = "found two agents fixing the same bug in two branches"
responsibilities = ["scope", "state"]
commands = ["start", "check"]
claims = ["scoped-task", "overlap-report", "scope-enforcement"]
+++
## The moment

Two sessions, two branches, the same failing test. Both find the cause, both fix it, both
open a change. One is wasted, and merging the other now conflicts with it.

## Why it happens

Each worker knew its task and nothing about the other's. Giving every worker its own git
worktree feels like the fix, and it does stop them overwriting each other's files. It does
not stop them doing the same work. In the environment this tool comes from, forty fully
isolated worktrees still produced several thousand concurrently modified files and dozens
of duplicated patches; nine of eighty ever merged. Isolation converted immediate overwrites
into deferred, larger conflicts. The mechanism that actually prevents duplication is a
declared scope that the other worker can see — and there, the declaration was optional,
matched by exact string equality, and enforced by hooks that never ran.

## What Majordomus does

`majordomus start` requires a scope: the paths this task may touch. It is normalised at the
time of declaration and stored in the task record. Every other worktree of the repository is
read straight from `git worktree list`; if one has an active task whose scope contains or is
contained by yours, `start` says so, naming the worktree and the path. `majordomus check`
and `majordomus finish` fail on any touched file outside the scope. One task is active per
checkout; a second `start` is refused until the first is handed over or finished.

While this site was being built, two sessions shared one checkout. The tool refused the
second writer's push until scopes were declared, and the two sessions moved to separate
worktrees — which is what the design asks for.

## What it does not do

Overlap is reported, never blocked: whether two people should work on overlapping paths is a
coordination decision, not a rule. It sees worktrees of one repository on one machine, not
other clones. It does not merge anything and emits no merge commands.

## Try it

```bash
majordomus start "fix flaky auth test" --scope lib/auth,test/auth
majordomus check --overlap
```
