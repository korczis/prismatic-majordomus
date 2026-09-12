---
id: project.a-number-names-one-case
version: 1
kind: rule
title: A case number names one case
description: The identity a behavioural case carries names it — `bash test/run.sh 96_...` runs it, `test/cases/96_*` selects it, and every rule, claim and commit message that says "case 96" means it. A new case takes its identity from a survey of every place one can already be claimed, and no two files of one tree share an identity beyond the collisions recorded on the day the gate was written.
statement: A new case takes the identity `scripts/case-numbers next` reports and announces it before writing it; no two files under test/cases/ share the text before their first underscore, except the files recorded on .ai/repo/ci/case-numbers-baseline.txt.
status: active
class: blocking
depends_on: [project.work-is-claimed-before-it-is-built@1, project.a-verdict-states-its-subject@1]
tags: [tests, identifiers, coordination]

x-majordomus:
  tests: [test/cases/291_a_case_number_names_one_case.sh]
---

# Rationale

Three things assume the identity names the case: `bash test/run.sh 96_worktree_topology`
runs one by name, `test/cases/96_*` is how a gate or a script selects one, and "case 96" is
how every rule, claim, ADR and commit message in this repository refers to one. When two
files carry an identity, the first is ambiguous, the second selects more than was meant, and
the third points at two things and the reader cannot tell which.

Measured on 2026-09-12, across 812 refs: 265 distinct case files had ever been added under
`test/cases/`, carrying 172 identities; 55 of those identities had been carried by more than
one distinct file. The tree of `origin/master` held 206 files under 164 identities, and 25 of
those identities were held by two to seven files at once — `97` by seven. Three sessions
took `278` within minutes of each other that day, and a session that told its peers "284 is
next" was wrong twice over: `283` was on a rescue ref and `290` on an open branch.

That is not carelessness. A session allocates from what it can see, and what it can see is
the tree it branched from — the one place the next free number is least likely to be. The
decisions had the same defect and `majordomus adr next` answered it: a survey of the working
tree, every sibling worktree on disk, every ref, and the peer board, each saying what it
claimed. Case identities are the second identifier allocated by hand here, and this rule
gives them the same survey and a gate for the half a gate can see.

# Required behaviour

**Allocating.** A new case takes the identity `scripts/case-numbers next` reports: one above
the highest identity any surveyed source claims. The allocation is monotonic and never
recycles a hole, because a hole is a number taken and withdrawn or taken and not yet
written, and no survey can tell those apart. The identity is announced — the number, not
only the path — before the file is written (`project.work-is-claimed-before-it-is-built`).

The survey is a best reading and says so. A ref scan sees only what was pushed; the board
sees an announcement only while its connection lives, and is unreachable more often than
not. A number two sessions take in the same minute, neither pushing nor announcing, is
invisible to both, and no mechanism removes that: the two gaps are complementary. So
`next` names every source it reached and every source it did not, lists the identities
allocated and not landed on the base with who holds each, and never presents its number as
a reservation (`project.a-verdict-states-its-subject`).

**Holding.** No two files under `test/cases/` share the text before their first underscore.
That text is taken verbatim, so `87` and `87b` are two identities and not a collision: a case
and its deliberate second half is a convention this repository already uses.

The collisions of the day the gate was written are recorded in
`.ai/repo/ci/case-numbers-baseline.txt` and may remain, because they are spread across
other sessions' branches and rescue refs, and renumbering them would break every one of
those. **Nothing may be added to it.** A file joining a collision the baseline already
carries is refused like any other, or the ratchet would slow the rot rather than stop it.
Shrinking the list needs no permission: `check` names each entry that no longer collides.
Rewriting the baseline to turn a refusal green is the one thing it is not for.

# Failure behaviour

`scripts/case-numbers check`, registered as the `case-numbers` gate, exits 10 naming each
new file, the identity it shares and what it shares it with, states the population it
examined, and names `scripts/case-numbers next`. `--strict` refuses every collision,
baseline or not, for whoever is paying the debt down.

`scripts/case-numbers next` exits 0 whenever it answered, including when the board or the
refs could not be read — each is then reported as not surveyed, never silently omitted.
`scripts/case-numbers collisions` makes the cross-ref state visible: every identity more
than one file has ever carried, whether each file is on the base, and who carries it now.

# Verification

`test/cases/291_a_case_number_names_one_case.sh` builds the repositories it measures:

- the gate over trees that are distinct, that collide, whose collision is on the baseline,
  whose baselined collision a third file joins, whose collision was renumbered away, and
  `87` beside `87b`;
- the allocator over a repository whose highest identity is only in a sibling worktree's
  uncommitted file, whose next highest was added and deleted on a rescue ref and is carried
  by nothing, whose unmerged branch holds an identity the base lacks, whose base renamed a
  case that older branches still carry, and whose branch carries a different file under an
  identity the base holds — each of which a survey of fewer sources answers wrongly;
- a board document announcing an identity above every file, and a base that does not
  resolve, each of which must be said rather than absorbed.
