+++
title = "One branch, one place"
description = "Several sessions working on one repository each need a checkout of their own. Why this repository takes away the choice of where a worktree goes, how the one place is derived from git, and how a worktree is moved or removed without losing anything."
template = "page.html"
[extra]
lang = "en"
related = ["claimed-before-it-is-built/", "a-session-ends-the-work-does-not/", "written-once-derived-everywhere/"]
[extra.translations]
cs = "jedna-vetev-jedno-misto/"
+++
{% raw %}

Several sessions, people and agents, work on this repository at once, and each needs a checkout of its own branch. Git's linked worktree is the mechanism, and git takes the destination as an argument, so every caller decides where a worktree goes. Before there was a rule, fifty-three of them had decided differently here: flat siblings named after nothing, session scratch directories, one inside the primary checkout, and one occupying the container's own path.

The disorder was not the cost. The cost was that "where is branch X" and "where does a new one go" had no answer that did not involve looking, guessing or inventing. A session that has to invent a path leaves its work where the next session does not look.

## One answer, derived from git

[ADR 0021](@/adrs/adr-0021.md) (accepted) removed the decision. Given the repository and a branch name there is exactly one path: the primary checkout's sibling, named after it with `-wt`, and below it the branch name with its hierarchy kept. The primary checkout is read from git's shared metadata, never from the current directory. The answer is therefore the same from the primary checkout, from a linked worktree and from four directories deep inside one. Asking the current worktree instead is the easy mistake, because inside a linked worktree it would produce a container per worktree, nested for ever.

A branch name is treated as input. It is validated by git's own reference rules, the derived path is proved to lie strictly below the container on every call, and two names that differ only by case are reported, because a case-insensitive filesystem would put both in one directory. The trunk is discovered rather than hardcoded. The suffix is written once in the code, and there is no setting for it, because a container that could be configured is one that two checkouts of the same repository could disagree about.

So a session starts a branch with `majordomus worktree create <branch>` and works in the path it prints. A path is never an argument. A handover can name the branch, and the session that resumes it finds the worktree without being told where it is.

## Refused where it matters

The rule is [project.worktree-topology](@/rules/project-worktree-topology-1.md), and it is enforced in layers that say plainly what each can do. The provider bootstraps tell every worker to establish the topology before implementing, and the task lifecycle starts by asking for it. The pre-commit hook asks `majordomus worktree guard`: a feature branch is committed only from its canonical worktree, and the primary checkout only on the trunk. `majordomus doctor` proves that the hook asks it, and the topology is shown on the command line, over MCP and HTTP and in the Cockpit, so a wrong one is visible everywhere.

The manual is honest about the limits. A git hook can be bypassed, and a CI runner cannot see anybody's local disk, so the CI gate measures the rule's machinery rather than the directories of one machine. Within the workflows Majordomus controls, a wrong worktree is refused. Outside them, it is visible.

## Moved without losing anything

A worktree already in the wrong place is brought home. `majordomus worktree migrate --plan` lists each misplaced worktree, where it belongs, how it would move and what blocks it, and `majordomus worktree migrate` makes the moves. Each move is checked by a fingerprint taken before and after it: the branch, the commit, the index, the staged and unstaged changes, and every untracked file with its content hash. A step is reported as moved only when the two fingerprints are equal. Nothing is reset, stashed, cleaned, checked out or deleted, and a destination that already exists is a conflict named by its kind, never overwritten.

## Removed only when it is safe

The disk is what runs out, so spare worktrees are reclaimed. `majordomus worktree cleanup` lists every branch merged into the trunk whose worktree is clean or absent, and `--remove` removes those worktrees and nothing else. Branches are never deleted, because a branch is the only durable name a piece of work has.

The listing only nominates. Immediately before each removal the branch's standing is read again, and the removal is refused for a branch that is ahead of its remote or has none, for a worktree with a process working inside it, and for uncommitted work. A modification time is not trusted. On 15 September 2026 two worktrees that looked untouched since the 12th had live processes in them.

The cost is one command before each new branch and a path nobody gets to choose. In return every session, and every surface, knows where every branch is worked on. The whole mechanism, from the derivation to the failure modes, is in [`docs/WORKTREES.md`](@/docs/worktrees.md).
{% endraw %}
