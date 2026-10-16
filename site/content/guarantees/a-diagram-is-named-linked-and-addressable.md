+++
title = "Every Mermaid diagram of the built site gets an id (diagram-<name>), the list of its nodes, the viewer's controls and a link for each node that is the identity of an object the site has a page for, all derived at build; a link to"
description = "A diagram on the site is a  mermaid  fence in a document, or Mermaid source a template"
weight = 263
[extra]
claim_id = "a-diagram-is-named-linked-and-addressable"
status = "guaranteed"
source = "docs/claims/a-diagram-is-named-linked-and-addressable.md"
+++
{% raw %}

## What it means

A diagram on the site is a ` ```mermaid ` fence in a document, or Mermaid source a template
writes from generated data. After the build, each one:

- has an id, `diagram-<name>`, so `#diagram-<name>` lands on it with or without JavaScript;
- lists the nodes its source declares, in a `data-nodes` attribute;
- has the viewer's toolbar beside it, written `hidden`;
- has beside it a link for every node whose id or label is the identity of an object this
  build wrote a page for: an issue, a milestone, a decision, a rule, a doctrine, a command,
  a capability, a document.

Nobody types any of these. A link to one node, `#diagram-<name>:<node>`, is resolved at
build against the nodes the diagram declares, and a link to a node that does not exist is a
finding.

## How it works

`scripts/site-build` runs `scripts/lib/diagrams.mjs` over the pages Zola wrote, before any
other producer's pages are composed in. The name is the `data-diagram` a template declared,
else the id of the heading the diagram sits under, else its ordinal on the page; a second
diagram under one heading is `<name>-2`. The identities and their routes are read from
`site/data/generated` (`plan.json`, `entities.json`, `commands.json`, `executable.json`,
`doctrines.json`, `docs.json`), and an identity whose page the build did not write is not
offered. A label that names two objects, or one the site has no page for, is linked to
neither.

The links are ordinary markup, so `scripts/ci/link-check` decides each of them with the rest
of the site's. The same check reads the `id` and `data-nodes` the pass wrote to resolve a
node address.

## How to see it

```bash
node --test scripts/lib/diagrams.test.mjs
bash test/run.sh 3006_diagrams_are_interactive_and_linkable
```

The case builds a site of its own with three diagrams on one page, runs the pass and
`scripts/ci/link-check` over it, and then plants four defects: a link to a node the diagram
does not declare, a link to a node that belongs to another diagram on the same page, a link
into a diagram the page does not carry, and two diagrams declared under one name. Each must
be refused with a finding that names it. It also requires an issue whose page was not built
to stay unlinked, the diagrams' sources to be unchanged, and a second run to change no
file.

## What it does not cover

The claim is about what the build writes and what `link-check` decides. That the viewer
(`site/diagrams.js`) zooms, pans and opens an address in a browser is exercised by the
`diagram` spec of `scripts/interaction-probe`, which needs a browser and is not part of this
claim.

Nodes are read for flowcharts and state diagrams. Any other kind of diagram is named and
addressable as a whole and has no node links or node addresses. A Mermaid syntax the parser
does not read costs a missing link, not a wrong one, except where a label happens to equal
an identity it does not mean.

Links from outside the site to a diagram's address are not protected: renaming the heading
a diagram sits under changes its address.

## Why it exists

Rule `project.a-diagram-is-drawn-not-typed` moved every published relationship into Mermaid
because a parseable diagram can be linked and checked. Until ADR 0152 nothing linked or
checked one: a node that was an issue or a command was text, on a site with a page for
each, and a diagram had no address to cite.
{% endraw %}
