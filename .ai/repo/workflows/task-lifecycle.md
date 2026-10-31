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

On a repository whose mesh is enabled, `start` claims the scope on the mesh as the session
`task-<id>`, and first ensures this checkout's shared server (`serve ensure`, bounded, said on
stderr), because a fresh worktree has none. A start that could not claim reports `WARN mesh`
with the reason; repair it without finishing: `majordomus serve ensure`, then `majordomus
check`, which claims the running task's scope. Until then `finish --outcome completed` and the
pre-push `finish --check` refuse it (rule `majordomus.task-on-the-mesh`, `docs/MESH.md`).

A task binds every push of its checkout: the pre-push hook runs `finish --check` against it,
scope and mesh included. So start one only in the checkout whose work it is, never in a
worktree whose pushes are another seat's, such as a train's, to hold a reasoning record or
any other record; hold it under your own worktree's task. On 2026-10-28 a task started in
the worktree of `integration/train-10b-20261028` only to hold a reasoning record made the
train's push refuse on scope and mesh. If one was started there, close it with a
non-completed outcome and its `# Next Action` before that checkout pushes.

Name in `--scope` every file the change will edit before the first edit, the test cases it
pins among them: a scope cannot be widened while its task is active, so a missed case costs
a `finish --outcome partial` with a note and a second `start` (I4340, 2026-10-29).

`start` refuses while a task is active here: hand it over or finish it first. `check` and
`finish --outcome completed` fail on a touched file outside the claimed scope; the other
outcomes name the files as warnings and close the record, because `start` will not begin a
task while one is active and refusing every outcome would strand a worker whose scope
turned out too narrow. A path the repository declares derived — git's `merge=derived`
attribute, which `scripts/gitattributes` writes from the generators — is admitted without
being in the scope when the task also changed a file of the layer or a file inside its
scope that is not derived: a scope names what a worker changes, not where a generator puts
its projections. Projections committed with no such change beside them are refused as
before. `finish --outcome completed` evaluates every line of the finish
contract the policy selects and writes nothing when any line fails; the other outcomes are
honest statements that the work did not complete, and each needs a note saying what comes
next (`# Next Action`) or why (`# Reason`). `no_match` means the thing sought does not
exist; `failed` means the work could not be done. They are different facts.

The work between `start` and `finish` follows the repository's skills, each read with
`majordomus skills explain <id>` or as the resource named here: a change to the tree follows
`majordomus://skill/implement`; removing a file, branch, worktree, record or process that
this task did not create follows `majordomus://skill/assess-before-deleting`; the final
report and every handover body follow `majordomus://skill/report-verification-state`.
What a task learned does not end as a note: before the episode closes, or whenever the
briefing names candidates awaiting review, follow `majordomus://skill/evolve` — decide each
candidate, put each lesson where it changes behaviour, deliver, and tell the peers.

## Answering "is it done?"

When a person asks whether the work is done — merged, deployed, no debt added, no more
branches, worktrees or pull requests than at the start — run `majordomus check` and report
its `done` line: every question of the done invariant still owed, as `id=status`, with the
evidence for each in `gates.completion`. Three of those questions are the repository's
closing readings, measured from where the task started: `no-new-debt` (the entries of every
`.ai/repo/*-baseline.txt`), `nothing-accumulated` (branches and worktrees created since and
still there) and `backlog-not-grown` (pull requests opened since and still open). Report
what the reading says, including `unknown`, and only then what you know on top of it; never
answer from what you remember doing (`project.a-closing-question-is-answered-by-measurement`).

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
