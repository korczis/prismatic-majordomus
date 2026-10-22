+++
title = "Work on a branch in the one worktree it belongs to"
description = "Start a branch without choosing a path, find every checkout of the repository from any of them, and bring a stray one home with its uncommitted work intact."
weight = 12
[extra]
id = "work-on-a-branch-in-its-canonical-worktree"
source = ".ai/repo/use-cases/work-on-a-branch-in-its-canonical-worktree.md"
category = "workers"
maturity = "described"
+++

## Situation

Several sessions work on one repository at once, each on its own branch, each needing its
own checkout. Git's linked worktree is the mechanism and git takes the destination as an
argument, so every session decides where its checkout goes; after a week nobody can say
where branch X is checked out, an agent told to work on a branch picks a plausible directory
and leaves an afternoon of work where the next session does not look, and the checkout that
happens to occupy the one sensible name blocks everything else.

## What you run

- `majordomus worktree` (the Rust executable): which branch and worktree this is, whether that is where the branch belongs, what is uncommitted here
- `majordomus worktree create <branch>`: the branch from the trunk when it is new, the worktree at `<repo>-wt/<branch>`; the path is derived, never given
- `majordomus worktree migrate --plan` and `migrate`: every misplaced worktree with where it belongs; then the moves, fingerprinted before and after, uncommitted work included
- `doctor`: the pre-commit hook asks `majordomus-cli worktree guard`, and the policy's enforcement list declares it, so `doctor` proves the wiring on every install
- `context`: the briefing names the branch and the worktree the records belong to, so a resumed session starts where the work is

## Scenario

```yaml
setup: installed-wired
given:
  - 'the layer installed and the hooks wired; the shell tool proves the wiring, the executable proves the topology'
steps:
  - id: wiring-holds
    run: ['doctor']
    note: 'every declared enforcement entry is wired, the worktree guard among them once the policy declares it'
    expect:
      exit: 0
      stdout_contains: ['doctor: 0 failure']
  - id: the-briefing-names-the-checkout
    run: ['context']
    note: 'the briefing is about this worktree and branch and never about somebody else''s'
    expect:
      exit: 0
then:
  - 'the executable derives <repo>-wt/<branch> for any branch from git identity alone; test/cases/96_worktree_topology.sh drives it end to end'
  - 'a misplaced worktree migrates with its staged, unstaged and untracked work, verified by a fingerprint before and after'
```

## Outcome

A branch has one worktree and one path that every surface — the command line, the MCP
tools, `/api/v1/worktrees`, the Cockpit — derives the same way, so a session, a handover
and a reviewer all find the same checkout. A worktree somewhere else is a typed diagnostic
with a remedy, and the migration that repairs it proves it lost nothing. Nothing is
registered, nothing is configured, and adding a branch edits nothing.
