# The task lifecycle

One active task per checkout. A task claims the paths it may touch and the profile it runs
under; everything else is derived from Git and from what the task records.

```
majordomus start "<task>" --scope <paths> [--requires <tokens>] [--profile <name>]
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

A scope says where a worker may write. `--requires` says what the worker *owes*: tokens of
`share/obligations.yaml` — `implementation`, `tests`, `docs`, `generated`, `rules`,
`commit`, `push`, `target`, `pages`, `deploy`, `verify`. `check` reports each one,
`majordomus evidence --covers <token> --command <cmd>` discharges the ones a worker
records, and git or the publication probe settles the rest live at HEAD without a ledger
line. A token the vocabulary does not declare is refused at `start`, not at the moment you
try to discharge it. Declare what the work owes when you begin it: a task that promises
nothing is told, truthfully and uselessly, that it promises nothing.

`start` refuses while a task is active here: hand it over or finish it first. `check` and
`finish --outcome completed` fail on a touched file outside the claimed scope; the other
outcomes name the files as warnings and close the record, because `start` will not begin a
task while one is active and refusing every outcome would strand a worker whose scope
turned out too narrow. `finish --outcome completed` evaluates every line of the finish
contract the policy selects and writes nothing when any line fails; the other outcomes are
honest statements that the work did not complete, and each needs a note saying what comes
next (`# Next Action`) or why (`# Reason`). `no_match` means the thing sought does not
exist; `failed` means the work could not be done. They are different facts.

Never author identity fields. `repository_id`, `branch`, `head`, `working_tree` and
`changed_files` on any record are computed from Git; a body that carries them is refused.

Escalate capability and effort only when the profile allows it and record that you did.
Think as hard as the task needs and report as briefly as the profile says: execution
depth is not output verbosity.

## Reading a gate's answer

Two failures cost most of a day on 2026-09-09, and both are about believing the wrong part
of what a tool said.

**A zero exit code is not a verdict.** `scripts/derive` exited 0 while printing an error
that two decision records shared one identity and both had been dropped from the index.
Everything derived afterwards was written without them. Derivations and their checks now
run under `--strict`, so an error diagnostic ends the run — rule
`project.diagnostics-decide-the-exit`. When you read a log, read the diagnostics, not only
the last line.

**A green history is not a green tree.** Master carried two defects that failed every branch
merging it, and its own history looked clean, because its last *completed* validate run
predated the commit that introduced them and every run since was queued. Before concluding
that a failure on your branch is inherited, check when the newest completed run on the base
actually finished, and if the answer is "there isn't one", run the gate locally. The full
gate, not an approximation of it: `--integration` passed on the branch whose `--ci` was red.

**Before adding a public command**, run `scripts/ci/command-furnished`. It reports in one
second everything the command still needs — its `docs/CLI.md` section, its fixture, its
`demo:`, its behavioural case and its negative case — instead of one missing piece per
ten-minute derivation, which is what it cost the day it was written.

**Values come from closed sets.** A use case's `target`, a rule's `exit_code`, a use case's
`category`, a scenario's `setup`: each is a list somewhere in the repository, and each was
guessed wrong at least once that day. Read the list. `majordomus usecase validate` and
`adr check` will tell you, but they tell you one derivation later than the file does.
