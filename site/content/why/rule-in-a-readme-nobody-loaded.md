+++
title = "The rule for that directory, in a README no session ever loaded"
description = "A local rule is either in the always-loaded file, where every session pays for it, or beside the code, where nothing relates it to the path being edited."
weight = 90
[extra]
id = "rule-in-a-readme-nobody-loaded"
status = "stable"
source = ".ai/repo/why/moments/rule-in-a-readme-nobody-loaded.md"
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

## Why a better model does not fix it

The worker cannot read a file it was not given and does not know exists. This is a
retrieval problem with a deterministic answer — which documents govern this path — and
deterministic answers should not be delegated to inference.

## What it costs

A defect that review has to catch, every time, for as long as the rule stays invisible. And
a reviewer who responds by moving the rule into the always-loaded file, which fixes this
directory and taxes every session in the repository.

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
set from git and reports which documents and scopes it touches. Rules have the same shape:
`majordomus rules list` prints the effective set, resolved as a dependency graph, and says
of each one whether the tool enforces it or nobody does.

## Before and after

```text
before   CLAUDE.md (root)                     loaded, says nothing about payments
         lib/payments/README.md               governs the change, loaded by nobody

after    $ majordomus context resolve lib/payments
         .ai/README.md                        final,  order 10
         .ai/repo/README.md                   extend, order 20
         lib/payments/README.md               extend, tracked by id ai.payments
```

## How to verify it

Add a context document for a directory and run `context resolve` on a path under it: the
document appears in the chain, in order. Break a reference in it and `context validate`
fails the whole tree rather than resolving part of it.

## What it does not do

It composes the documents for a path; it does not make the worker read them. The generated
instruction file tells the worker to resolve the context for a path before working under
it, and the briefing obeys a line budget and names every section it dropped. It does not
parse a provider's own nested-file conventions: the resolution here is what applies, and the
provider's loading is treated as an optimisation.
{% endraw %}
