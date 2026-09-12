+++
title = "The briefing an episode opened with is compared against git before it is believed"
description = "The briefing an episode opened with is compared against git before it is believed"
weight = 123
[extra]
kind = "rule"
slug = "project-the-briefing-is-checked-against-git-1"
identity = "project.the-briefing-is-checked-against-git@1"
status = "active"
source = ".ai/repo/rules/project/the-briefing-is-checked-against-git.v1.md"
+++
{% raw %}

## Rationale

This repository has one vocabulary for the distance between a recorded head and the checkout
in front of you — `exact`, `advanced`, `diverged`, `different_context` — and it applies it
almost everywhere. The task record is labelled before the context builder prints a line under
it. The resolved handover is labelled, and so is the checkpoint. The derivation labels the
head it started from. The closed session record is labelled by `session show`, and there is a
whole case about that one. The pattern is deliberate: a recorded head is a claim about a
history, and a claim about a history is worth exactly as much as its agreement with the
history in the working tree.

The working context of an open episode was the exception. It carries `head:` and `branch:` in
its front matter like every one of its neighbours, and nothing compared them. That is worse
than it sounds, because of what this particular document is. It is not an artefact a worker
looks up when it wants one — it is the briefing, resolved once by the provider's start hook
and delivered into the worker's context at the moment the episode opened, and then never
mentioned again. Every other recorded head in this repository is read by somebody who chose
to read it. This one is read by somebody who did not, and cannot re-read it, and has no way
of noticing that the repository moved on without it.

So a session could work for an afternoon from a description of a tree that no longer existed,
and `context`, `session context` and `doctor` would each stay quiet, because each of them was
checking the relation in every direction but this one. The measured instance: a briefing
frozen at one head while the trunk had moved twice past it, with nothing anywhere saying so.

The failure is not staleness. Staleness is the normal life of a briefing — it becomes
`advanced` the instant its own session makes a commit, and `different_context` the instant
the worker moves to the branch it was told to build. The failure is a one-way check: a
relation this repository verifies for every sibling artefact and left unowned for the one
artefact nobody chose to read.

## Required behaviour

**The comparison exists and is named.** A surface that hands a worker the working context of
an open episode states the distance between the head that document recorded and the head of
the checkout, in the five words above. `unknown` is the word for a document that carries no
head or no branch; a briefing that cannot be compared must never read as a current one.

**Drift is reported, never refused.** No check fails an episode for a briefing that has
advanced or moved to another branch. A doctrine that did would stop every commit in this
repository, and it would be wrong on the facts as well: the briefing is behind because the
work went well.

**A distance is a number where a number means something.** `advanced` carries the count of
commits gained since the freeze, because "your briefing is four commits old" can be acted on
and "your briefing is stale" cannot. `diverged` and `different_context` are not distances and
carry none.

**The document is not rewritten to make the report come out fresh.** The working context is
frozen evidence of what the worker was told, and a store that rewrote its own history
underneath the prompt that consumed it would be worth less than one that admits its age. What
was missing was the age, not a mutation. Re-resolution is what `majordomus context` is for,
and its output supersedes the briefing rather than replacing it.

**No second word for stale.** The vocabulary comes from `mj_git_label` and nothing here
invents a parallel one. A reader who has learned what `advanced` means for a handover has
learned what it means for a briefing.

## Failure behaviour

The surfaces report; nothing refuses. The one state that fails is `unknown` under the
`majordomus.session-lifecycle` doctrine, because a document this tool wrote always carries
both fields and their absence is the producer having broken rather than the repository having
moved.

## Verification

`test/cases/127_briefing_freshness.sh` opens an episode, asserts that all three surfaces say
`exact` and that none of them warns, advances the trunk by two commits and asserts that each
reports `advanced` with the count, checks out another branch and asserts `different_context`
with a warning from the context builder and no failure from the doctrine, strips the head
from the document and asserts that the doctrine fails on `unknown`, and moves the document
aside to assert that absence is still reported as absence. The negative assertions carry the
case: a guard that warned about every briefing would satisfy the positive ones while telling
a worker nothing.
{% endraw %}
