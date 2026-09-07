---
schema: feature/v1
id: knowledge
kind: feature
title: Decisions and knowledge that compound instead of evaporating
short_title: Knowledge
headline: What was decided, why, and what was rejected is recorded where the next session reads it, and a durable decision becomes an architecture record with typed links to what it put in force.
summary: Decisions are recorded with their reason and their task, open questions block acceptance until resolved, architecture decision records carry typed references to the rules, claims, files and tests they put in force, and the graph of all of it is derived from those references rather than drawn.
status: stable
weight: 60
featured: true
areas: [decisions, context]
modules: [graph]
commands: [decision, adr, knowledge, search, history, question]
kinds: [adr, knowledge, document]
rules: [majordomus.adr-integrity, majordomus.decision-records, majordomus.decision-threshold, majordomus.externalise-decisions]
docs: [docs/CONTINUITY.md, docs/CONCEPTS.md]
adrs: [adr-0010, adr-0020]
claims: [decision-record, decision-attribution, adr-catalogue, adr-traceability, adr-propose, record-search, semantic-retrieval, history-ledger-read]
use_cases: [record-a-decision-before-it-is-forgotten, keep-decisions-out-of-the-transcript, read-back-what-happened, find-an-object-without-reading-everything]
cockpit: [graphs, objects]
related: [continuity, doctrine]
tags: [decisions, adr, knowledge]
---

## What it does

`majordomus decision add` records what was decided, why, what was rejected and which task
decided it, superseded later by an entry that names it and never edited. `majordomus adr`
lists, shows, proposes and checks the architecture decision records under the layer; a
record extracted from a local decision is proposed, never accepted in the same act, and its
`related` field carries typed references — a rule, a claim, a file, a test — that the graph
turns into edges, so the reverse question, what decided this rule, is derived rather than
written twice.

Knowledge is declared, not collected: the source classes say which files carry it, the
index reads them through git, and `search`, `knowledge` and `history` answer over that index
without anyone reading everything.

## What it does not do

Nothing is promoted automatically. A decision recorded while working stays in the checkout's
local state until a person judges it durable enough to write down as a record. There is no
embedding, no vector store and no summariser; retrieval is over declared identities and
text, and a reference that resolves to nothing is a finding, not a phantom node.
