+++
title = "Assess before deleting"
description = "Remove a file, branch, worktree, record or process only after each candidate has been classified by what depends on it and what would be lost, and the reversible step has been taken first."
weight = 1
[extra]
id = "assess-before-deleting"
status = "active"
version = 1
source = ".ai/repo/skills/assess-before-deleting/SKILL.md"
+++
{% raw %}

## Purpose

A removal is the one change a later commit cannot revert: a deleted untracked file, a
dropped worktree holding uncommitted edits, a branch whose commits reached no remote, a
process another session was relying on. In a repository many workers share, "this looks
unused" is a guess about other people's work. This skill turns the guess into a
classification with evidence, and puts the reversible step before the irreversible one.

## When to use

Before deleting anything you did not create in this session: a file or directory, a local
or remote branch, a linked worktree, a stash, a record under the layer, a build directory
another checkout may share, a server or a background process. Also before a bulk operation
whose selection is a pattern rather than a list.

Not this skill: removing a file your own uncommitted change just added, or regenerating a
projection (regenerate it; never delete one by hand).

## Procedure

### 1. Name every candidate

List the candidates explicitly, one per line, with the identity a command can act on: a
repository-relative path, a full ref name, a worktree path, a record id, a process id and
its full command line. A pattern is not a candidate; expand it first and read the list.

### 2. Classify each one

Answer, for each candidate, with the command whose output is the evidence:

<div class="overflow-x-auto" tabindex="0">

| question | evidence |
|---|---|
| Is it tracked, generated or local? | `git ls-files`, the generated-file header, `majordomus scope <p>` |
| Does anything reference it? | `git grep -n <name>`, `majordomus context resolve <p>`, the index resource for it |
| Does another worker claim it? | the peer board (`majordomus_peers`), `scripts/collision-check --new <p>` |
| Does it hold work reachable nowhere else? | `git log --oneline <ref> --not --remotes`, `git status` in the worktree, `git stash list` |
| Is a process doing something for someone? | the full command line and working directory, never a name match |

</div>


A merged branch can still hold unfinished work in its worktree; zero unique commits is not
the same as nothing to lose. A process matched by name may belong to another session.

### 3. Grade the risk

- **low** — derived or reproducible, referenced by nothing, claimed by no one.
- **medium** — local work that exists elsewhere (pushed), or a reference that can be updated
  in the same change.
- **high** — work reachable nowhere else, a claim by another worker, or any answer above you
  could not obtain. Not knowing is high.

### 4. Take the reversible step first

Push the branch or create a rescue ref (`git branch rescue/<name> <sha>` and push it) for
anything that holds work; copy what is ignored and valuable out of the tree before removing
the tree; stop a process by its own id, never by a pattern. A high-risk candidate is not
removed in this session unless its owner says so in words; record it instead.

### 5. Remove, then verify

Remove the low and medium candidates, one class at a time. Afterwards run what would notice
a mistake: `majordomus doctor`, the cases covering the touched paths, `git worktree list`,
the peer board. A removal nothing checked is not finished.

## Output

The removal table, one row per candidate:

<div class="overflow-x-auto" tabindex="0">

| candidate | class | evidence | risk | action | verification |
|---|---|---|---|---|---|

</div>


followed by the rescue refs created, the candidates left in place with the reason, and the
verification commands with their exit codes. Report each row with the state the
`report-verification-state` skill defines: a removal whose verification did not run is
PARTIAL, not done.
{% endraw %}
