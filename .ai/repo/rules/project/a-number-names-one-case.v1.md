---
id: project.a-number-names-one-case
version: 1
kind: rule
title: A case number names one case
description: The number a behavioural case carries identifies it — `bash test/run.sh 96_...` runs it, `test/cases/96_*` selects it, and every rule, claim and commit message that says "case 96" means it. A number already held by another case may not be taken again, and the identifier is claimed in the tree because the tree is the only place every session can see it.
statement: No two files under test/cases/ share the text before their first underscore, except the ones recorded as accepted debt on the day the gate was written; a new case takes a number no case holds.
status: active
class: blocking
depends_on: [project.work-is-claimed-before-it-is-built@1]
tags: [tests, identifiers, coordination]
---

# Rationale

Three things assume the number identifies the case: `bash test/run.sh 96_worktree_topology`
runs one by name, `test/cases/96_*` is how a gate or a script selects one, and "case 96" is
how every rule, claim, ADR and commit message in this repository refers to one. When two
files carry a number, the first is ambiguous, the second selects more than was meant, and
the third is simply wrong — it points at two things and the reader cannot tell which.

Measured on 2026-09-10: nineteen numbers on `master` were held by two or more files, and
`97` by five. That is not one afternoon's accident. It is the shape of an identifier that no
mechanism ever checked, in a repository where parallel sessions are the normal case.

The way it happens is worth stating, because it is not carelessness. A session allocates a
number from what it can see, and what it can see is the tree it branched from. Two branches
of a single fan-out took `110` within the same minute; neither had been pushed, so no branch
scan could have found the other, and both were right by every check available to them. The
repository already knows this shape — `project.work-is-claimed-before-it-is-built` says a
branch scan sees what was pushed and the peer board sees what is being written now, and that
neither sees a number two sessions are taking in the same minute.

The answer is not to coordinate harder. A peer board holds one announcement per connection
and loses it when a worker reconnects, so it understates what is held; a session that starts
tomorrow never read it at all. **A number claimed in the tree and refused by a gate is
visible to every session, including the ones that never spoke to anybody.**

# Required behaviour

No two files under `test/cases/` share the text before their first underscore.

That text is taken verbatim, so `87` and `87b` are two numbers and not a collision: a case
and its deliberate second half is a convention this repository already uses, and a rule that
argued with it would be wrong rather than strict.

The debt of the day this was written is recorded in
`.ai/repo/ci/case-numbers-baseline.txt` and is allowed to remain, because fifty files
cannot be renumbered under the sessions that are mid-flight in them. **Nothing may be added
to it** — a new file that joins a collision the baseline already carries is refused like any
other, or the ratchet would only slow the rot rather than stop it. Shrinking the list is
welcome: a renumbering removes its entry and needs no permission.

# Failure behaviour

`scripts/ci/case-numbers-unique`, registered as the `case-numbers` gate, exits 10 naming
each file, the number it shares and what it shares it with, and names the command that shows
which numbers are free. `--strict` refuses every collision including the baseline's, for
whoever is paying the debt down.

# Verification

`test/cases/122_case_numbers_unique.sh` builds the trees the gate must tell apart in
fixtures of its own: numbers that are all distinct, a second file taking one, the baseline
recording that debt and accepting it, a third file joining the same collision and still
being refused, a renumbering leaving a baseline entry that no longer collides and passing
anyway, and `87` beside `87b` passing under `--strict`.
