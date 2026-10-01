---
schema: adr/v1
id: adr-0010
kind: adr
title: Curated knowledge is one kind with a class, not a second store
status: proposed
date: 2026-09-05
tags:
  - knowledge
provenance:
  origin: extracted
  derived_from:
    - decision:t-20260905034523-a9f1
    - file:share/schemas/majordomus/knowledge/knowledge.v1.schema.json
related:
  - file:share/kinds.yaml
  - file:.ai/repo/knowledge/sources.yaml
  - test:test/cases/64_knowledge_discovery.sh
  - test:test/cases/73_knowledge_nodes.sh
---

# 10. Curated knowledge is one kind with a class, not a second store

## Context

The layer already discovers everything it holds the same way: `.ai/repo/knowledge/sources.yaml`
names a source class, a kind and one pathspec, and `majordomus knowledge sources` reports every
tracked file that matches, with the class that claimed it. Policies, scopes, rules, decisions,
sessions and use cases all arrive through that one mechanism.

Notes a repository writes about itself — what to read first, a convention somebody settled, a
constraint the tooling cannot express, a lesson paid for once — did not fit any existing kind,
and they are the kind of thing a project accumulates continuously. The obvious move was a second
store: a directory of notes with its own reader, its own front matter and its own discovery, kept
beside the layer rather than inside it.

The question this decision answers is not where the notes live. It is whether a new *sort* of
knowledge earns a new *mechanism*, or whether it is a value in a field of the mechanism that
already exists.

## Decision

Curated knowledge is the `knowledge` kind, discovered by the `curated` source class like every
other kind, and the distinction between a fact, a convention, a constraint, a memory and a lesson
is carried by a required `class` field inside the record.

`share/kinds.yaml:192` declares the kind: Markdown, front matter required, schema
`majordomus.knowledge/v1`, identity `[id]`. `.ai/repo/knowledge/sources.yaml:108` declares the
class: `kind: knowledge`, `discovery: vcs`, pathspec `:(glob).ai/repo/knowledge/curated/*.md`.
`share/schemas/majordomus/knowledge/knowledge.v1.schema.json:51` makes `class` required with a
closed enum — `fact`, `convention`, `constraint`, `memory`, `lesson` — beside the other required
header fields, of which `status` (`candidate`/`verified`/`superseded`) and `epistemics`
(`observed`/`inferred`/`decided`) say how far the statement is to be trusted and where it came
from.

Nothing about a curated note is special to the reader. Its node kind comes from the source class
that discovered it, not from anything in the file: `test/cases/73_knowledge_nodes.sh:91` asserts
exactly that, against a fixture whose body talks about rules while its class says otherwise.

## Alternatives rejected

**A second store with its own reader.** A `notes/` directory outside the layer's source classes,
read by its own code path. It was rejected because every consumer the layer has would have needed
a second one. Discovery is one list (`knowledge sources`), and the graph is built over what that
list returns, so a store outside it is invisible to `knowledge nodes`, `knowledge edges`, the
index the Rust executable builds, the site projections derived from that index, and every gate
that reads any of them. The cost is not the reader; it is that each of those grows a special
case, and a query that is true of the layer stops being true of the repository.

**A kind per sort of note** — `fact`, `convention`, `constraint` each a kind of its own. That
keeps one discovery mechanism but multiplies the schemas, the allow-lists generated from them and
the source classes, and it makes reclassifying a note a rename plus a move rather than an edit of
one field. The sorts also share every other field: a lesson and a convention differ in what they
claim, not in what they carry.

**Leaving the sort implicit**, in prose or in tags. Rejected because a reader cannot narrow on it:
`knowledge nodes --kind` selects on kind, and with one kind for all curated notes the class is
what remains to select on. A tag would do the same work with no closed set and no schema to refuse
a typo.

## Consequences

A curated note is subject to the whole of the layer's discovery contract, not a corner of it. It
is discovered through the version-control index, so an untracked file is not knowledge, and the
`:(glob)` prefix means `*` never crosses a directory separator — `test/cases/64_knowledge_discovery.sh:73`
asserts that no file is discovered by two classes, having been written after dropping that prefix
made every handover under `.ai/local/` arrive as shared repository knowledge.

The class is a closed enum, so adding a sixth sort of note is a schema change with a regenerated
allow-list (`share/allow/knowledge.txt`, generated from the same schema by `majordomus generate`),
not a new directory. That is the intended friction: the cost of a new sort is one review of
whether the five existing ones really fail to hold it.

Every field a curated note carries is one the rest of the layer already understands —
`provenance.origin` and `provenance.derived_from` with typed references that must resolve, and
`relations` with the layer's own relation types — so a note participates in the knowledge graph
rather than sitting beside it, and a note that cites a file which no longer exists is a finding of
the same check that judges every other record's references.

What this forecloses is a per-sort schema: a constraint cannot grow a field that a lesson does not
have without every knowledge record gaining it as optional. Where a sort needs its own shape, that
is evidence it is a different kind, and the question returns to `share/kinds.yaml`.

One gap is worth stating plainly, because the decision's own contract implies more enforcement
than the tree has. The schema declares `class` required, and the generated allow-list refuses an
*unknown* key — but a record with the key *missing* is still reported as a node by
`majordomus knowledge nodes`, since the reader takes the node's kind from the source class. So
"every curated note declares its class" is today a property of the two records that exist rather
than one a gate holds.
