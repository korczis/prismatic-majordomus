+++
title = "One branch, one worktree, at a path derived from git"
description = "The container is the primary checkout's sibling named with -wt, a branch's worktree is the branch name under it with its hierarchy kept, both derived from git identity and registered nowhere; a pre-commit guard refuses a feature branch committed from anywhere else, and migration moves a misplaced worktree with its uncommitted work, fingerprinted before and after."
weight = 40
[extra]
id = "worktrees"
status = "stable"
source = ".ai/repo/features/worktrees.md"
+++
{% raw %}

## What it does

Given the repository and a branch name there is exactly one path, and every surface derives
it the same way: `majordomus worktree create <branch>` makes the branch from the trunk and
the worktree at that path, `worktree path` prints it for a shell to `cd` into, `worktree`
alone says where the current session is and whether that is where it belongs, and
`worktree migrate` brings a misplaced worktree home with its modified, staged and untracked
files intact. Nothing registers a path: git's own common directory, its worktree list and
its refs are the registry.

The pre-commit hook asks `worktree guard` and refuses a feature branch committed from the
primary checkout or from a worktree that is not the branch's own. The topology is one
capability module, so the same typed answer — every worktree with its standing, every
diagnostic with its code and remedy — is on the command line, over MCP and HTTP, and on the
Cockpit's worktrees page.

## What it does not do

It never deletes a branch, never overwrites a destination that exists, and never moves a
detached worktree or a session's scratch checkout unless asked. It does not decide what a
branch is for; the issue a branch names is read from its name when it names one.
{% endraw %}
