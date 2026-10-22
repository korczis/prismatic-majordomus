---
schema: adr/v1
id: adr-0053
kind: adr
title: An identity is allocated above the high-water mark, and a collision is refused on the branch
status: proposed
date: 2026-09-12
tags:
  - coordination
  - governance
provenance:
  origin: authored
related:
  - rule:project.work-is-claimed-before-it-is-built
  - rule:project.blocking-checks-cheap
  - rule:project.empty-is-not-failure
  - rule:project.no-network-no-eval
  - test:test/cases/271_adr_allocation.sh
---

# 53. An identity is allocated above the high-water mark, and a collision is refused on the branch

## Context

On the night of 2026-09-11 three branches of this repository claimed decision identity 0044
and two claimed 0043. Every duplicate makes `majordomus generate --strict` exclude *every*
claimant of that identity and refuse the whole tree, and the error it prints names the
identity, not the two sessions that took it — so the defect surfaces at merge time, hours
from either cause, to a third person who has to decide which of two finished documents to
throw away.

Three separate sessions each hand-rolled the same survey that night to find their way out —
every ref, every worktree, the board — independently and correctly. Nothing offered it. The
survey already existed in this tool, inside `mj_adr_next_id`, reachable only by proposing a
decision; no one could ask the question without also writing a file.

The repository already has a rule for this, `project.work-is-claimed-before-it-is-built`,
which says to claim an identifier before building on it. **That rule could not be followed as
written.** `scripts/collision-check`, the mechanism it names, reads *committed* refs; the
sessions that collided had not pushed. The peer board, which the rule's own prose names as
the other half, is a process's memory and nothing read it for identities. A rule whose
precondition is unobservable at the moment it applies is the class of defect this repository
exists to remove.

## Decision

**Allocation is monotonic.** `majordomus adr next` hands out one above the highest identity
anything has ever claimed, and never recycles a hole in the sequence. A hole means an
identity was taken and withdrawn, or taken and not yet written; no survey can tell either
from a number nobody ever used. 0034 is the standing instance — it is on no branch, in no
tag and in no working tree, and master's own ADR 0035 and `docs/ENTRY_AUDIT.md` cite it. So a
gap is reported as *spent, and not free*, with whatever cites it named as evidence for the
reader. The citation search is evidence, never the mechanism: the hole is spent whether or
not anything is found to cite it, which is what keeps the rule decidable.

**The survey names its sources and its denominator.** Four, and each reports what it
claimed: this working tree, every sibling worktree as it stands on disk, every ref
(branches, tags, remote-tracking refs and every linked worktree's HEAD, in one `git log
--all --no-renames --diff-filter=A`), and the peer board of the shared server. The board is
the only source that knows about an identity a session has decided to take and has not
written anywhere — that is how 0034 was reserved for `feature/peer-board-survives-restart`
while existing as no file. A source that could not be reached says so; it is never silently
dropped from the denominator, because the reason three sessions each wrote their own survey
was that no answer here carried one.

**A collision is refused on the branch, at commit time.** `majordomus adr check` now asks the
other refs a question no single tree can answer about itself: for every identity this tree
adds relative to the base, does another ref carry a *different* document there? It names both
documents, both refs and the command that finds a free number. `doctor` runs the same
examination, and `doctor` is the pre-commit hook, so the refusal lands where the answer is
still cheap to act on. The CI gate `adr-collision` runs the same command; there is one
inventory of decision identities, not two.

Different means a different file name. Two refs holding the same path is an edit, and an edit
is resolved by merging; two refs holding `0044-cooperation-is-…` and `0044-the-model-catalogue-is-…`
is a choice between two finished pieces of work. Only the second is refused.

`project.work-is-claimed-before-it-is-built` stays **advisory**. Its identifier half is now
mechanically decidable and gated; its path half is not, and must not be — two implementations
arriving at one module is sometimes the right thing to accept deliberately, and nothing can
decide that mechanically. Promoting the whole rule because half of it became enforceable
would make the tool refuse a judgement that belongs to a person.

## Alternatives rejected

**A registry of allocated identities.** A file listing which numbers are taken. It is a
second inventory of something the tree already states, it goes stale the moment a branch is
abandoned, and two sessions editing it collide exactly as they collided over the ADRs — the
defect, relocated.

**Recycling gaps.** Handing out 0034 because no file sits there. It is what a naive survey
does, it is what makes two correct surveys of the same evidence disagree, and it would have
broken ADR 0035's citation. A sparse sequence costs nothing.

**A new script beside `adr check`.** `scripts/ci/adr-collision-check` was the obvious shape
and it would have been a second reader of the decisions directory, drifting from the first.
The examination belongs inside the command that already owns ADR integrity, where `adr
check`, the `adr-integrity` doctrine, the pre-commit hook and the CI gate all reach the same
code.

**A second network call site for the board.** `SECURITY.md`, `project.no-network-no-eval` and
`test/cases/08_no_forbidden_constructs.sh` each name `lib/context.sh` as the single declared
exception, and hold it to its shape in both directions. The board fetch was factored into
`mj_peer_board` in that file and called from the allocator: one call site, two callers.

**Blocking on a board claim.** The board is read by the allocator and not by the gate. A
gate whose verdict depends on which processes happen to be attached is not reproducible in
CI, and `project.blocking-checks-cheap` asks a blocking check to be deterministic. The board
informs the allocation; the refs decide the refusal.

## Consequences

The shell tool has no peer identity of its own — it reads the board over HTTP, and the board
answers sessions rather than commands — so a session that announces an allocation and then
runs `adr next` is stepped over its own claim. That costs one identity out of a sparse
sequence and is the safe direction; the output says the high-water mark came from a board
claim so the reader can recognise it.

The survey's cost is one `ls-tree` of the base per run, and one `git log` over the decisions
directory — measured here at a fifth of a second across every ref and the whole history
(`git for-each-ref | wc -l` says how many that is) — and the second of those runs only when
this tree actually adds an identity. Against what `adr check` already spends parsing the
records it is not measurable; `MJ_TIMING=1 majordomus adr check` is what says so on any
given tree.

A gate now fails on a branch that renumbers a decision onto an identity another branch
already published. That is the intended cost: the alternative is the same failure, later,
against `generate --strict`, with the two sessions no longer identifiable.

`--no-renames` is load-bearing in the ref survey. Rename detection is on by default, and a
decision whose slug was edited arrives as an `R` rather than an `A`; without the flag the
survey undercounted this tree's own refs, and an undercounting allocator is worse than no
allocator. `majordomus adr next` prints what each source claimed, so the failure is visible
rather than inferred.
