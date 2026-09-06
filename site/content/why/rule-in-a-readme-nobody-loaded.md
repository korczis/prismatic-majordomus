+++
title = "The rule for that directory, in a README no session ever loaded"
description = "Why one always-loaded file cannot carry every local rule, and how a tree of scoped documents is composed for the path a worker is about to touch."
weight = 9
[extra]
hook = "found the rule for that directory in a README no session ever loaded"
responsibilities = ["layer", "policy"]
commands = ["context", "rules"]
claims = ["context-documents", "context-impact", "ai-layer-manifest", "context-selection-budget", "rule-resolution", "vendored-rule-package"]
+++
{% raw %}
## The moment

The payments directory has a README that says every amount is an integer in minor units,
and why. A session adds a float. The reviewer points at the README. The session had loaded
the root instruction file, which says nothing about payments, and nothing told it that a
second document applied to the path it was editing.

## Why it happens

There are two bad places to put a local rule. In the always-loaded file, where it costs
every session context whether or not the session touches payments — that is how one such
file in the source material grew to eleven hundred lines. Or in a README beside the code,
where it costs nothing and is read by nobody, because no mechanism relates the path a worker
touches to the documents that speak for it. Providers that load nested instruction files
each do so in their own way, for their own file names, and none of them will say which
documents were in force for a given change.

## What Majordomus does

Context is a tree, not a file. Under the `.ai/` layer a directory carries a context
document, true for that directory and everything below it, and the layer is one directory
whose manifest names every section, readable without the tool. `majordomus context resolve
<path>` prints the chain from the root down to that path, in one deterministic order, and
`context explain` says why each document is in and why each filtered one is out. A document
can also declare which source paths it tracks, so the document about payments is found from
`lib/payments` although it does not live there. The nearest document adds to its ancestors
and never replaces them.

`context validate` fails the whole tree on a broken reference, a cycle, an unknown key or an
illegal override, and an invalid tree resolves nothing. `context affected` reads a change
set from git and reports which documents and scopes it touches — including a tracked source
that changed, which means its document is due for review. Rules have the same shape:
`majordomus rules list` prints the effective set, the vendored baseline plus the project's
own rules, resolved as a dependency graph, and says of each one whether the tool enforces it
or nobody does. The baseline is vendored with a manifest of hashes, and a hand edit to it is
detected and refused.

## What it does not do

It composes the documents for a path; it does not make the worker read them. The generated
instruction file tells the worker to resolve the context for a path before working under
it, and the briefing obeys a line budget and names every section it dropped. It does not
parse a provider's own nested-file conventions: the resolution here is what applies, and the
provider's loading is treated as an optimisation.

## Try it

```bash
majordomus context resolve lib/payments
majordomus context explain lib/payments
majordomus context affected --staged
majordomus rules list
```
{% endraw %}
