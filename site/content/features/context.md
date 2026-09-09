+++
title = "Every directory of the layer carries its contract, and a worker reads what applies"
description = "Every README under .ai/ is a context document with an identity, a scope, the providers and audience it addresses and how it composes with its ancestors; the repository scope declares what a worker reads and what it never reads; and the briefing a worker gets is assembled from durable state within a line budget with every exclusion named."
weight = 140
[extra]
id = "context"
status = "stable"
source = ".ai/repo/features/context.md"
+++
{% raw %}

## What it does

`majordomus context resolve <path>` prints the documents that apply to a path, least
specific first; `context explain` says why each is in or out; `context validate` refuses a
tree with a missing contract, a duplicate identity, a broken or cyclic override, or an
override of a document marked final; `context affected` names the documents a change set
touches. The repository scope is declared once, out over in, and the executable discovers,
indexes and serves nothing outside it.

## What it does not do

A provider's own nested-file loading is an optimisation the tool does not replace; the
resolution is what applies. The checkout-local half of the layer is never context and never
published.
{% endraw %}
