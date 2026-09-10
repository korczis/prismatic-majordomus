---
id: project.reclaim-only-what-you-own
version: 1
kind: rule
title: A worker reclaims only what it owns
description: Build output, scratch trees and worktrees belong to whoever is working in them; a worker under disk pressure reclaims its own and says what it removed, because everything else on the machine is another worker's instrument.
statement: A worker removes build output only from the worktree it is working in, and never from the primary checkout or another branch's; disk pressure is reported, not resolved by taking someone else's tools.
status: active
class: advisory
depends_on: [project.worktree-topology@1]
tags: [process, worktrees]
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

The economics also do not favour the sweep. Reclaiming a colleague's build costs them ten
minutes of rebuild to save one worker a few seconds of waiting, and the saving is usually
imaginary: the volume had a hundred and eighty gigabytes free within the hour, because the
worker who owned the problem solved it deliberately.

# Required behaviour

**A worker removes build output only from the worktree it is working in.** Never from the
primary checkout, never from another branch's worktree, never from a scratch tree it did not
create.

**Disk pressure is reported, not resolved by taking somebody else's tools.** A worker that
cannot proceed says so, names what it needs, and lets the pressure be resolved by whoever can
see every worker at once.

**Reclaiming is classified before it is done.** Build output is regenerable and may go; a
worktree may not be removed wholesale, because a branch whose history is fully merged can still
hold authored files that were never committed. What is deleted is named in the report.

**A borrowed instrument is verified before it is trusted, and it may vanish.** A worker that
borrows a built executable establishes that it matches the tree before using it, and treats its
disappearance as an environment fact rather than as a failure of the branch under test.

# Failure behaviour

Decided by review and by the record on the shared board. No gate can distinguish a deliberate
reclamation from a careless one, and inventing one would be worse than the disease — it would
have to forbid a worker from cleaning its own tree.

The observable symptom is worth knowing by name, because it will happen again while this rule
is only advisory: a tool that was present a minute ago and is absent now is an environment
event, and the first move is to rebuild and re-measure rather than to file a defect against
whatever was being tested at the time.

# Verification

Held by review. The related mechanical facts — that a worktree belongs at a derived path, and
that the executable is located rather than assumed — are held by the worktree guard and by the
launcher's own resolution.
