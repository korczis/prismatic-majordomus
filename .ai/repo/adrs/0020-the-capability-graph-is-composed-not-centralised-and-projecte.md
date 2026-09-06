---
schema: adr/v1
id: adr-0020
kind: adr
title: The capability graph is composed, not centralised, and projected twice
status: proposed
date: 2026-09-06
tags:
  - architecture
  - capabilities
  - interface
  - web
related:
  - rule:project.interfaces-are-projections
  - rule:project.no-new-nouns
  - rule:project.derived-once
  - rule:project.hot-path-reads-once
  - file:apps/majordomus-cli/src/graph.rs
  - file:apps/majordomus-cli/src/capability/builtin/graph.rs
  - file:apps/majordomus-cli/src/cockpit/mod.rs
  - file:docs/COCKPIT.md
provenance:
  origin: authored
---

# 20. The capability graph is composed, not centralised, and projected twice

## Context

ADR 0012 settled how the Cockpit renders: it is a projection of the registry, laid out from
what a capability answered, holding no inventory of its own. ADR 0013 settled how surfaces
are served: each is discovered from the producer that owns it and resolved once. Both
decisions are about the machinery. Neither says what the picture is *of*.

That question was left with an answer nobody chose. The executable knows capabilities from
the registry and objects from the index, and each graph derivation reads one side: the
registry graph draws modules and their capabilities, the layer graph draws directories and
kinds, the rules graph draws rule dependencies, the decisions graph draws what a decision
put in force. Everything the layer holds that is not a capability — the rules, the skills,
the decisions, the documents, the use cases, the benchmarks, and after the deployment work a
deployment — sits beside the capabilities and is joined to them only in a reader's head.

The failure that produces is not that a page looks sparse. It is that a capability with no
documentation, no test and no enforcement is indistinguishable on every surface from one
that has all three, because nothing resolves the relation between them. The repository can
already say what exists. It cannot say what is finished.

The references themselves were never missing. A rule names what it depends on, a decision
names what it put in force and what it stands in for, a use case names the commands it runs
and the claims it evidences, a context document names what it tracks. Each of those is
front matter with a schema and a validator. What was missing was a model that read them as
one kind of thing, and a verdict when one of them named something that was not there.

Two repairs suggest themselves immediately, and both are the shape this repository has
refused before. The first is a manifest: one file listing every capability with its
documentation, its tests and the rule that governs it. The second is a browser application
that fetches the API and assembles the picture on the client. The second one also decides
the harder half of the problem by accident, because the published site has no server behind
it: a picture assembled from a running process is either absent or fabricated on a static
page, and a reader cannot tell which.

## Decision

**The graph composes what each registry owns; it centralises nothing.** The `composed`
derivation reads the capability registry and the index, both of which remain authoritative
for exactly what they were authoritative for before, and joins them into one typed model. It
names no kind of its own: the node kinds are read off what was actually indexed, so a kind
the layer gains appears in the graph without this derivation being edited. An object's
identity in the graph is its URI, which survives a retitle; a label never is.

**The layer's reference conventions live in one table and are read twice.** `RELATIONS`
states what a front matter field means when an object of some kind declares it, and it is
the only place that is written down. The composition reads it to draw edges;
`unresolved_relations` reads the same resolution as a verdict. There is no second opinion
about what a reference means, and no way for the drawing and the checking to disagree.

**Inference is over stable identities and nothing else.** A reference resolves through an
identity, a declared id or a repository-relative path. It never resolves through a title or
a substring, because a title is edited for clarity and a substring matches by accident. A
versioned identity resolves by its stem, since a reference names the rule rather than the
version it was written against. Where a convention cannot be inferred safely, the reference
is declared — and the table follows the layer's convention rather than correcting it: the
first two findings this check produced on this repository were both defects in the table,
not in the layer, because skills name their siblings by bare identity.

**A boundary and a defect are different things.** A `file:` or `test:` reference that leaves
the layer draws an external node, which is what an edge out of the layer looks like. A
reference that names a kind this repository holds and resolves to nothing is a finding
carrying the file, the key, the reference and the correction — and it is not drawn. An edge
to a phantom renders as an empty section, and an empty section reads as an answer.

**Definitions and runtime state are separate types.** `Graph` holds definitions.
`RuntimeState` is an overlay keyed by node id, laid on by a consumer that has a process to
ask. It is deliberately not a field of `Node`: a node able to carry runtime state carries it
into the static projection, where nobody can refresh it and a reader cannot distinguish it
from a current value.

**Both projections read that one model.** The static projection is the graph as generated;
the runtime projection is the same graph with the overlay on top. This is what lets the
published site render the whole architecture with no backend, and it makes parity between
the two a test rather than a hope — strip the runtime-only fields and the representations
must be equal.

**Availability is metadata, not a condition inside a page.** Whether a thing means anything
without a running backend is a field of the model. No projection may decide it from a
hostname, an address or a build flag, because a rule written into a template is a rule
nobody finds when it is wrong, and the failure it produces is a link on a published page
with nothing behind it.

**The browser stays an enhancement, exactly as ADR 0012 already requires.** JavaScript adds
filtering, the relation view, live state and search. It is not required to read a
documentation link, and the relations are in the served HTML before it runs.

## Alternatives rejected

*One central manifest of capabilities, their documentation, their tests and their rules.* It
is the fastest thing to build and the easiest thing to read, and it is the second registry
this repository already refuses for rules, kinds and capabilities. It is correct on the day
it is written and wrong on the first rename nobody propagated, and the drift is silent
because a manifest cannot notice that a rule it names has been deleted.

*A client-side application fetching the API and assembling the graph in the browser.* It
holds the model in a second place, needs a build toolchain at runtime, and makes the first
paint depend on JavaScript — all of which ADR 0012 rejected for the Cockpit and none of
which improved. The decisive objection is the published site: with no server to fetch from,
the same code must either render nothing or render something it cannot have obtained, and
the second is worse than the first.

*A separate generator for the static projection.* It would be quick, and it is the second
universe this repository dismantled once already. Generation belongs to the canonical target
set, where the drift check can see it; a generator outside it is a projection nobody
verifies.

*Inferring relations from titles or substrings, so contributors declare nothing.* It works
until two subsystems name something the same way, and then it draws an edge that is wrong in
a picture that looks authoritative. Ambiguity is a declaration, not a heuristic.

*Drawing an unresolved reference as a node so the graph stays complete.* A phantom node
makes a broken reference look like a boundary, which is the one distinction this model
exists to make.

*Letting a node carry its own runtime state, with the static build simply leaving it empty.*
An empty field and a stale field are indistinguishable to a reader, and the type would make
the wrong thing easy.

## Consequences

A subsystem that wants to appear has two obligations and no third. It registers itself in
the registry that already owns its kind — a capability in `capability!`, an object under its
canonical directory — and it names its relations in its own front matter, in the fields the
schema already declares. Nothing else is edited: not the navigation, not the overview, not
the search index, not the graph, not the coverage matrix.

The cost of not doing it is exact, and worth stating because it is not a failure. A
subsystem that registers itself but names no relations appears as a node with no edges: it
is listed, it is searchable, and the coverage matrix reports it as undocumented and untested
because nothing resolves to say otherwise. That is the honest answer, and it is the reason
inference is not extended to guess a relation from a name. A subsystem that names a relation
that does not resolve is refused: generation stops with the file, the key and the correction
rather than publishing a page with a section that goes nowhere.

The strictness has a price that will be paid by somebody in a hurry. A reference typo in
front matter now fails a build that would previously have rendered a slightly wrong page.
That is the trade this decision makes deliberately, and the message is the part that has to
stay good: it names the declaration, the missing end and the repair, so the failure is a
correction rather than an investigation.

Composition reads the index once into the lookups every reference needs, because resolving
the layer's references by scanning the objects per reference is the shape
`project.hot-path-reads-once` exists to forbid. The graph grows with the repository, so its
derivation is a benchmark target like every other operation and its cost is measured rather
than assumed; `majordomus bench` states the number this prose deliberately does not.

The coverage matrix that follows from these edges will read as an indictment on its first
run, and it is not one: it is a baseline measurement of a repository that has never had the
relation resolved before. Saying so is part of shipping it.

Nothing here narrows ADR 0012 or ADR 0013. The Cockpit remains a projection and holds no
inventory; web surfaces remain discovered from their producers and resolved once. What
changed is the subject: what those surfaces project is now one composed model of everything
the repository knows, rather than several one-sided derivations that each saw half of it.
