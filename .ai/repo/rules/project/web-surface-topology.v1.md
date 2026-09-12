---
id: project.web-surface-topology
version: 1
kind: rule
title: A web surface is discovered from its producer, never registered twice
description: Everything this repository exposes over HTTP is one resolved topology, discovered from producers and projected to serving, publication, validation and documentation from that one resolution.
statement: A web surface is declared once, by the producer that makes it, and every consumer — the router, the static publication, the validator, the listing and the documentation — reads the same resolved topology; adding a conventional surface changes only its producer.
status: active
class: blocking
depends_on: [project.derived-once@1, project.interfaces-are-projections@1]
tags: [web, architecture, projections]

x-majordomus:
  tests: [test/cases/84_web_surfaces.sh]
---

# Rationale

Before this rule the repository knew its web surfaces in as many places as it had consumers:
four prefixes named individually in the router, one output directory in the publication
model, path classes in the gate model, and nothing that could name both worlds. Two more
surfaces were arriving, and added the old way each would have cost a route, an output path,
a path class, a copy step, a line in a documentation table and a mention in the deployment
workflow. Six registrations for one directory of HTML is the definition of an abstraction
that leaks, and the seventh surface costs the same again.

# Required behaviour

A static surface is a directory under the generated web root with a `surface.json` beside
it: the producer writes both, and that declaration is the whole registration. The mount is
the one thing nothing can infer — a directory of HTML does not say whether it belongs at
`/tests` or at `/reports/tests` — so it is declared, by the producer, and everything else
follows from discovery.

Nothing else registers a surface. Not the router, which asks the resolved topology whether a
static surface owns a path; not the publication, which composes by mount; not the gate
model, the documentation, or a list anywhere. A route the executable answers itself is
described in the topology so that collisions are caught, and dispatched by the executable,
because those routes differ in behaviour rather than in data.

Route ownership is validated, never left to the order routes were added in. Two surfaces may
not claim one mount; a surface may not sit inside another's subtree, the root application
excepted, since answering what nothing else claims is its whole job; a static surface's
directory lives under the generated root; and a request may not leave the directory it is
served from.

The generated web root is derived state: reproducible, disposable, never tracked and never
read as a source.

# Failure behaviour

`majordomus web validate` reports every finding with the surface, the value, where the value
came from and the fix, and exits 10 on any error; the `web-topology` gate runs it in CI, and
a change that leaves the topology unresolvable does not merge. A surface whose directory is
missing is a finding under `--artifacts`, which is what serving and publishing need, and not
one before the producer has run, which is what a validation before a build can honestly say.

No command decides whether a surface *should* exist, or where it belongs: that is intent, a
person states it in the producer, and this rule only holds the repository to stating it once.

# Verification

`test/cases/84_web_surfaces.sh` proves it with a surface nobody wrote code for: a directory
with a declaration is listed, explained with its provenance, validated, selectable by its
declared id, composed into the publication under its mount and served by the running
executable — and removing it leaves nothing behind. The refusals are proved by mutation: a
mount claimed twice, a surface nested in another's subtree, a declaration with a key the
contract does not have. The unit tests of `apps/majordomus-cli/src/web/` hold the model,
the composition and the serving boundary. ADR 0013 records the decision and what it rejected.
