---
id: project.entities-are-routable
version: 1
kind: rule
title: Every object of the layer has an address of its own, and every kind says where it is published
description: A decision, a rule, a skill or a command that can only be read by opening a file in the repository is not part of the system that reads it. Every object the index holds has a deterministic address derived from its kind and identity, every surface resolves that one address, and no consumer carries a list of entities, kinds or routes of its own. A kind added to the layer reaches every surface because it was added, not because someone remembered to register it.
statement: Derive an object's address from its kind and identity, resolve it on every surface from the one index, and declare for every kind where it is published; a per-entity registration in a router, a catalogue, a navigation array or a documentation index is a second inventory and is forbidden.
status: active
class: blocking
depends_on: [project.derived-once@1, project.interfaces-are-projections@1]
tags: [governance, routing, projections, web]

x-majordomus:
  tests: [scripts/ci/entity-check, test/cases/277_entities_are_routable.sh]
---

# Rationale

This repository already had the hard half. Every file under `.ai/` is indexed as a typed
object with a kind, an identity and a `majordomus://` URI; one relation table resolves every
reference an object declares and reports the ones that resolve to nothing; every object is
an MCP resource. What it did not have was the easy half, and the easy half is the one a
reader meets:

- **No address.** An object could be read at `/cockpit/object?uri=majordomus%3A%2F%2Frule%2Fproject.x%401`
  — a query parameter carrying a percent-encoded URI. That is a way to fetch a record, not an
  address to give someone. Nothing linked to it, nothing could be bookmarked, and the site
  published no page for a decision or a project rule at all.
- **Half a graph.** The relation table was read forward only. A rule said what it depends on;
  nothing answered *what depends on this*, although the edge that answers it was already
  there, pointing the other way.
- **No statement about publication.** Which kinds reach the public site was implicit in
  twenty lines of a shell generator. A kind could be added to the layer and reach no public
  page, and nothing anywhere would say so — the silent discovery failure this repository has
  met before, and the reason it now measures instead of assuming.

## What the rule requires

**An address is derived, never assigned.** The route of an object is its kind and its
identity reduced to one path segment by one deterministic function. There is no table
mapping objects to routes, because a table can be incomplete; the function cannot. It can,
however, produce the same segment for two identities of one kind, and that is a collision
rather than a tie-break: a route that answers with one of two objects is worse than no
route, so the gate refuses the tree while one exists.

**Both directions come from one declaration.** An object declares what it references. The
backlink is that same edge read from the other end. Nothing declares a relationship twice,
which is what makes it impossible to declare half of one.

**Every kind says where it is published.** `site/data/publication.toml` carries one entry per
kind of the index: published as entity pages at a route, published by an editorial section
already, or not published with the reason. The reason is not a formality — it is the
difference between a decision and a silence, and a silence is what lets a kind go unpublished
for a year.

**No consumer carries an inventory.** Not the Cockpit's router, not the command line, not the
API, not MCP, not the site's templates, not its navigation. If adding a rule, a decision or a
skill requires editing any of those, the projection is not a projection — it is a copy that
happens to agree today. The acceptance test for this rule writes one rule into the layer and
nothing else, and asserts that it reaches the index, the listing, its own page, the two
404s and the typed API with no consumer touched.

## What it does not require

It does not require that every kind be published publicly. A prompt, a scope declaration and
a mesh identity are facts about a checkout rather than about the product, and publishing them
would be noise with a URL. It requires that the decision be written down where a check can
read it.

It does not require that an entity page assert enforcement. What an object *names* as its
executable artefacts, and whether the tree holds them, is what a page may say; whether any of
it ever ran is a different question with a different answer, and the page points at the
capability that answers it rather than rounding up.
