# The task lifecycle

One active task per checkout. A task claims the paths it may touch and the profile it runs
under; everything else is derived from Git and from what the task records.

```
majordomus start "<task>" --scope <paths> [--profile <name>]
majordomus context            # what you need to know now; run this first, every session
   ... work ...
majordomus checkpoint         # progress, on stdin, at the profile's interval
majordomus check              # before claiming anything is done
majordomus handover < note.md # to continue in another session, or
majordomus finish --outcome <completed|partial|blocked|no_match|failed> --verify-command "<cmd>"
```

Before `start`, establish where you are. A branch's work happens in its canonical
worktree — `<repo>-wt/<branch>`, derived from git and never chosen — and the trunk's in
the primary checkout:

```
majordomus worktree                          # branch, worktree, canonical or not, uncommitted work
majordomus worktree create <branch>          # new work: the branch from the trunk, the worktree at its path
cd "$(majordomus worktree path <branch>)"
majordomus worktree migrate --plan           # out of place: what would move; `migrate` moves it, work included
```

The pre-commit hook asks `majordomus worktree guard` and refuses a feature branch committed
from anywhere else. A mismatch is corrected with these commands, never by continuing in the
primary checkout; `docs/WORKTREES.md` and rule `project.worktree-topology` say why.

`start` refuses while a task is active here: hand it over or finish it first. `check`
and `finish` fail on a touched file outside the claimed scope. `finish --outcome
completed` evaluates every line of the finish contract the policy selects and writes
nothing when any line fails; the other outcomes are honest statements that the work did
not complete, and each needs a note saying what comes next (`# Next Action`) or why
(`# Reason`). `no_match` means the thing sought does not exist; `failed` means the work
could not be done. They are different facts.

Never author identity fields. `repository_id`, `branch`, `head`, `working_tree` and
`changed_files` on any record are computed from Git; a body that carries them is refused.

Escalate capability and effort only when the profile allows it and record that you did.
Think as hard as the task needs and report as briefly as the profile says: execution
depth is not output verbosity.
