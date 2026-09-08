+++
title = "Decisions and knowledge that compound instead of evaporating"
description = "Decisions are recorded with their reason and their task, open questions block acceptance until resolved, architecture decision records carry typed references to the rules, claims, files and tests they put in force, and the graph of all of it is derived from those references rather than drawn."
weight = 60
[extra]
id = "knowledge"
status = "stable"
source = ".ai/repo/features/knowledge.md"
+++
{% raw %}

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
{% endraw %}
