---
schema: adr/v1
id: adr-0173
kind: adr
title: The knowledge base is published, one page per record, and each record is linked to what it is about
status: proposed
date: 2026-10-17
tags:
  - knowledge
  - site
  - graph
  - publication
related:
  - file:.ai/repo/adrs/0091-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md
  - file:site/data/publication.toml
  - file:apps/majordomus-cli/src/graph.rs
  - file:docs/KNOWLEDGE.md
provenance:
  origin: authored
---

# 173. The knowledge base is published, one page per record, and each record is linked to what it is about

## Context

`site/data/publication.toml` declared the `knowledge` kind `projection = "none"`, with the
reason that curated knowledge is written for the worker reading the layer and not for a
visitor. That held while the base was four orientation notes. On 2026-10-17 the owner asked
for the repository's knowledge base to be derived from every source that carries know-how
and to be wired in, in the UI and on GitHub Pages ("potrebuji vygenerovat a zapojit .ai
knowledge base ze vseho dostupneho ... zapojena, v ui, v gh pages"). Milestone
`knowledge-base` distils the commit history, the CI runs, the session records and the
lessons sessions recorded into observations, which the deriver turns into candidate records
(ADR 0091, ADR 0118). Those records are the traps, workarounds and defect classes of this
repository, each with a subject and evidence that resolves: what a contributor reading the
site needs before changing the thing it is about.

A record names its subject in `about`, and the composed graph drew no edge from it, so even
the Cockpit could not show, on a rule's page, what the repository has learned about it.

## Decision

1. The `knowledge` kind is published through the entity projection at `/knowledge/`: one
   page per record, generated from the record's own file, with its references and the
   objects that refer to it read from the composed graph. Nothing names a record in the
   site generator, and a record added to the layer gets its page on the next derivation.
2. The composed graph carries one relation for the kind: a record is `about` its subject.
   A path resolves to the object at that path or to a file node; `rule:` resolves to the
   rule; `capability:`, `command:` and `gate:` resolve to the capability node or to a
   named node outside the layer. A subject that resolves to nothing the index holds is a
   boundary, drawn as an external node, never a finding: whether the subject is right is
   the knowledge integrity validator's judgement, and a derivation that refused the whole
   graph over one candidate would stop every surface for a review that has not happened.
3. A candidate is published as a candidate. Its page carries its status, and the entity
   projection's status filter separates `candidate`, `verified` and `superseded`; nothing
   on the site calls an unreviewed record verified.

## Consequences

- The site's knowledge section grows with the review queue, not with the product. That is
  the point: the queue is the repository's own account of what it has learned.
- A record's text is public the moment it is tracked. The observations that reach the
  candidates directory carry no name of a person, no quotation, no account or credential
  detail; `knowledge check` refuses a record that carries a conversation.
- Promotion stays a person's act (ADR 0091). Publishing a candidate is not promoting it.
