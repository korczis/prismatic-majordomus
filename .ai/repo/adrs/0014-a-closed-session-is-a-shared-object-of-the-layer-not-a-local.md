---
schema: adr/v1
id: adr-0014
kind: adr
title: A closed session is a shared object of the layer, not a local note
status: proposed
date: 2026-09-06
tags:
  - architecture
  - records
related:
  - rule:project.never-store-transcripts
  - file:lib/session.sh
provenance:
  origin: extracted
  derived_from:
    - file:lib/session.sh
    - file:.ai/repo/rules/project/never-store-transcripts.v1.md
---

# 14. A closed session is a shared object of the layer, not a local note

## Context

This repository already closes an execution episode into an immutable record: `session
close` writes front matter carrying the identity, the profile, the branch, the head it
started from, the commits it produced, the references it touched and the outcome, derived
from git and from the ledger rather than from anybody's memory. It is the right artifact,
and almost nobody can read it, because it is written under `.ai/local/state/`, the half of
the layer that is never shared and never projected. The tool that can list every rule,
claim, decision and use case over four surfaces cannot list what happened last Tuesday.

The comparison that prompted this was a repository where the same need is met by a flat
directory of dated Markdown files, a naming convention, and a commit hook that checks the
file name. It works, and its weaknesses are instructive: the directory's own README keeps a
hand-written index of the files (it claims a retention of fifty, admits to a hundred and
seventy-nine, and holds five hundred and eighty-eight), the enforcement validates the name
and never the content, and two naming formats are accepted for compatibility with the
first. The principle is worth taking. The shape is not.

Taking the principle here runs into a rule this repository already holds:
`project.never-store-transcripts` — *no record stores or summarises a conversation; records
carry what is true now and the next action*. A session narrative written from a transcript
is exactly what that forbids, and the forbidding is correct: a summary of a conversation is
unverifiable, ages badly, and invites the model that wrote it to be the authority on what
happened.

## Decision

A closed session becomes a shared object of the layer, discovered like every other kind and
projected to every surface the registry feeds — the command line, the API and its OpenAPI
document, MCP, the site and the cockpit. The record moves from the checkout-local half to
the tracked section `.ai/repo/sessions/`, and a source class discovers it there; nothing
else registers it, which is what makes the projections free.

The rule is honoured rather than bent: a session record carries only what the repository
can prove. The identity and the times, the profile and the task it belonged to, the branch,
the head it started from and the head it ended at, the commits between them, the paths the
working tree changed, the outcome, and the references the episode produced. The optional
body is a person's or a worker's own summary of *the work*, which is a statement about the
repository, not a retelling of a conversation with a model. Nothing reconstructs dialogue,
quotes a prompt, or attributes reasoning to a turn — the prompts have their own store, and
it is local by design.

What does not cross the boundary stays behind. The absolute worktree path is a fact about a
machine, not about the repository, and is dropped rather than published. What the ledger
holds in detail stays in the ledger, which remains local: the shared record names what
happened, and the local account remains available to whoever holds the checkout.

Retention becomes git's problem rather than a pruning loop's. A tracked record is history;
the reason the compared repository needs a retention policy is that its store is a
directory nobody can query, and a store that every surface can query does not need to be
kept small enough to read by hand.

## Alternatives rejected

*Leaving the record local and projecting it from there.* The projections read the index, and
the index reads the tracked tree; teaching it to read the checkout-local half would make
every projection depend on one machine's state, which is the property that makes the local
half local.

*Writing a second, shareable record beside the local one.* Two records of one episode, kept
in step by whoever remembers. The repository has a rule about that too.

*Keeping the free-form dated Markdown of the compared repository.* A file name is not a
schema. Without one, nothing can validate that a record carries an outcome, that its head
is a commit, or that the paths it names exist — and the projections would be rendering
whatever somebody typed.

*Publishing the ledger itself.* It is an append-only event log with machine-local detail
and no editorial judgement; it is evidence for the record, not a substitute for it.

## Consequences

Session records become public in a public repository. That is a deliberate disclosure: what
this repository holds is what it did — which branches, which commits, which outcomes — and
the same transparency that makes the claims and the decisions readable applies to the work
that produced them. The schema is what keeps that safe, because it closes the field set: a
record cannot carry a secret it has no field for, and the one free-text field is a summary
of the work whose contract says so.

`session close` changes where it writes, so a worker's habit does not change and the next
`doctor` finds the record in the tracked tree. The section is a governed directory and
carries its contract like every other one, which the coverage check enforces from the day
it exists.

The cockpit, the site and MCP gain a kind without being told about it, which is the whole
argument for the registry, applied to the last part of the repository that was not yet
part of it.
