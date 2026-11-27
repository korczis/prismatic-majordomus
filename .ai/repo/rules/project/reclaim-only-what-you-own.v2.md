---
id: project.reclaim-only-what-you-own
version: 2
kind: rule
title: A worker reclaims only what it owns, and every deleting sweep asks one liveness predicate
description: Build output, scratch trees and worktrees belong to whoever is working in them; a worker under disk pressure reclaims its own and says what it removed, and every sweep that deletes asks the one liveness predicate, worktree.liveness, which keeps whatever it cannot prove nobody uses.
statement: A worker removes build output only from the worktree it is working in, and never from the primary checkout or another branch's; disk pressure is reported, not resolved by taking someone else's tools; and every sweep that deletes a tree or a build directory — worktree cleanup, scripts/reap-orphans, the build host's disk-guard — decides whether anybody uses it through the one capability worktree.liveness (`majordomus worktree liveness <path>`), removing nothing it answers keep for and nothing at all when its answer is incomplete or the executable is absent.
status: active
class: blocking
depends_on: [project.worktree-topology@1, project.destructive-sweeps-fail-closed@1]
tags: [process, worktrees, reclamation]

x-majordomus:
  tests: [test/cases/611_the_reclaim_refuses_what_it_cannot_prove.sh, test/cases/276_disk_is_bounded.sh]
---

# Rationale

This machine runs many workers at once, in linked worktrees of one repository, and a build
directory is expensive: a full one is several gigabytes, and twenty of them filled a
nine-hundred-gigabyte volume to ninety-four per cent in a day. So a worker that meets disk
pressure is right to want to clear something, and wrong about what.

On 2026-09-10 a built executable disappeared twice while it was being used. The first time it
was a worktree's build directory that another worker had been told to borrow from; the second
time it was **the primary checkout's**, minutes after it had been built and verified. Neither
removal was announced. Both produced the same symptom — `MAJORDOMUS_BIN is not an executable` —
which reads as a broken branch, and cost a full rebuild plus the time spent believing a test
failure that was an artefact of the missing tool.

The prior form of this is already written down for processes: a pattern-kill reads to everybody
else as an environment failure. Files are the same lesson with a longer fuse. A killed process
announces itself immediately; a deleted build directory announces itself later, to somebody
else, as a defect in their own work.

Version 1 stopped there, as advisory, because no gate could tell a deliberate reclamation from a
careless one. The sweeps then became automatic — `reap-orphans` runs before a build below the
reclaim mark, `disk-guard` runs on a timer on the build host — and each defined "in use" its
own way. On 2026-10-28 at 21:43 the reaper removed a live worktree's `target/`. It held the only
copy of the executable that worktree's hooks require under ADR 0224, and every hook refused with
exit 12 after it. The reaper had decided liveness from command lines and working directories
alone, and the session working there through absolute paths showed in neither: its builds and
its `MAJORDOMUS_BIN` pointed into the tree from another directory. A sweep is not a person; it
can be held to one predicate, and that is what this version requires.

# Required behaviour

**A worker removes build output only from the worktree it is working in.** Never from the
primary checkout, never from another branch's worktree, never from a scratch tree it did not
create.

**Disk pressure is reported, not resolved by taking somebody else's tools.** A worker that
cannot proceed says so, names what it needs, and lets the pressure be resolved by whoever can
see every worker at once.

**Every sweep that deletes asks one liveness predicate.** `worktree.liveness`, declared in the
worktree module and implemented once in `apps/majordomus-cli/src/worktree/liveness.rs`, answers
`keep` or `reclaim` for each path with every reason it found. `majordomus worktree cleanup
--remove` asks it in process; `scripts/reap-orphans --targets` and `disk-guard` ask the
executable (`majordomus worktree liveness <path>...`), the guard at the path
`disk-guard.service` hands it. No sweep keeps a definition of liveness of its own beside it.

**A path is live** when an open majordomus task is recorded in its tree; a live mesh claim or
active session names the branch's issue, the branch or the path; a peer is attached to or
announced on the tree's own server; the derive lock's live owner names the branch; an offload
run record or offload lock pid file names a process working in it; a running CI runner job's
home holds it; another process's `CARGO_*_DIR` or `MAJORDOMUS_BIN` points into it; another
process works inside it or names it; or the tree's index, `HEAD` or reflog changed within the
idle window.

**Not knowing keeps it.** A reading that cannot be taken — a claim source, a stat, a task
record — counts as live. A source that cannot be read at all makes the answer incomplete
(`complete: false`, exit 10), and a sweep given an incomplete answer removes nothing: a declared
mesh whose claims cannot be read removes nothing and the reaper exits non-zero; `disk-guard`
without the executable removes no build directory.

**Reclaiming is classified before it is done.** Build output is regenerable and may go; a
worktree may not be removed wholesale, because a branch whose history is fully merged can still
hold authored files that were never committed. What is deleted is named in the report, and so
is every reason something was kept.

**A borrowed instrument is verified before it is trusted, and it may vanish.** A worker that
borrows a built executable establishes that it matches the tree before using it, and treats its
disappearance as an environment fact rather than as a failure of the branch under test.

# Failure behaviour

Blocking. A sweep that removes a path the predicate answers `keep` for, that removes anything on
an incomplete answer, or that removes a build directory without the executable fails case 611
(`scripts/reap-orphans --targets` against a fixture repository: each kept target is kept with
the predicate's reason, a declared mesh whose claims cannot be read removes nothing and exits
12, a target with no signal at all is reclaimed) or case 276 (a hard-tier directory idle past
its window whose worktree's `HEAD` moved within it is kept by `disk-guard`, and `disk-guard` run
with no executable at the path its service names removes no build directory). The unit tests of
`worktree::liveness` plant every signal and the absence of all of them.

The choice of a person to clear their own tree is still reviewed on the shared board, not by a
gate. The observable symptom of a reclamation gone wrong is worth knowing by name: a tool that
was present a minute ago and is absent now is an environment event, and the first move is to
rebuild and re-measure rather than to file a defect against whatever was being tested.

# Verification

`bash test/run.sh 611_the_reclaim_refuses_what_it_cannot_prove 276_disk_is_bounded` and
`cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib worktree::liveness`. The related
mechanical facts — that a worktree belongs at a derived path, and that the executable is located
rather than assumed — are held by the worktree guard and by the launcher's own resolution.
