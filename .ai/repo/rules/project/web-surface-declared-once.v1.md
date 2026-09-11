---
id: project.web-surface-declared-once
version: 1
kind: rule
title: A web surface is declared once and every web projection is derived from it
description: Everything this repository exposes over HTTP is one resolved surface with a mount, a producer and provenance; the router, the landing page, the machine-readable index, the published tree, the generated reference and the benchmark targets are projections of that resolution, and no second list of routes exists anywhere.
statement: Declare a web surface once, where its producer is, and derive every router, page, index, publication and reference from the resolved topology; a mount written down a second time is a bug, and `/docs` belongs to the documentation while `/swagger` belongs to the Swagger UI.
status: active
class: blocking
depends_on: [project.derived-once@1, project.interfaces-are-projections@1]
tags: [architecture, web, routing]

x-majordomus:
  tests: [test/cases/89_web_surface.sh, scripts/rust-check, scripts/generate-site-data, scripts/site-basepath-check]
---

# Rationale

A path is a name, and a name written in two places is a name that will mean two things.
This repository has already paid for that once: the Swagger UI was mounted at `/docs`
because nothing was serving documentation yet, and the moment documentation arrived the
obvious path was taken and the obvious path was occupied. Nobody decided that an API viewer
should own the word "docs"; it happened because the mount was a constant in a router and
the meaning of the word lived only in the heads of the people who had read that router.

`project.interfaces-are-projections@1` already requires a capability to be defined once and
every interface derived from it, and the routes the executable answers itself obey it. What
that rule does not reach is the other half of the web surface — the generated directories,
the published site, the reports a producer writes — which are not capabilities and were
each known separately by the router, the publication script, the gate model and the
documentation. A surface added the way the earlier ones were cost a registration in every
one of those, and the registration a person forgets is not the router's, because a broken
route is noticed in a minute. It is the documentation's, and a route reference that lies is
noticed after it has been believed.

The topology is therefore resolved rather than listed. Every consumer reads the same
resolution, so a surface cannot appear in one projection and be absent from another, and a
surface removed from its producer disappears from all of them together instead of leaving a
navigation entry pointing at a path that no longer answers.

# Required behaviour

A surface is declared once, at its producer: a route the executable answers is declared in
the `capability!` its module already carries, a generated directory declares itself beside
its output, and the application's site is read from the site generator's own configuration.
There is no central manifest of surfaces, and a resolved manifest written for diagnostics is
disposable — nothing may read it as truth that could not be recomputed.

Every web projection is derived from that one resolution. The router dispatches by asking
the resolved topology which surface owns a request path, and never by naming a surface in a
match arm. The landing page at `/` is rendered from the topology, grouped by the category a
surface declares and narrowed to the surfaces marked for a person; a link on it is a mount
that was resolved, never a path written into a template. The machine-readable index, the
published tree, the generated route reference on the website and the benchmark targets are
projections of the same value. A route table maintained by hand — in Rust, in a template, in
Markdown, in JSON, YAML or TOML — is a defect regardless of whether it currently agrees.

Route ownership is validated rather than left to insertion order. A surface owns its mount
and everything below it; two surfaces may not claim one path within the same world; a
nested claim is an error unless the outer surface declares it. Validation refuses at
construction, before the process serves a request, rather than surfacing as a wrong answer
to a user. The registry describes the executable that is running: a surface whose runtime
capability this build or this invocation lacks is absent from the topology, and therefore
absent from the page and the index, rather than advertised and broken.

Two top-level namespaces are reserved and may not be reassigned. `/docs` is this
repository's own documentation and nothing else — it must never again be given to the
Swagger UI or to any other subsystem that merely finds the word convenient. `/swagger` is
the Swagger UI. `/openapi.json` is derived from the capability registry and is the document
the Swagger UI reads. Adding, removing or renaming a surface is a change to its declaration
and to nothing else; a change that also requires editing a page, a table or a manifest means
the projection it edits is not derived and must be made so.

# Failure behaviour

`majordomus web validate` reports a finding for a duplicate mount within one world, a nested
claim the outer surface did not declare, an unknown category or visibility, and a native
surface with no handler behind it; it exits `10` when any finding is an error. Router
construction refuses the same conditions rather than starting, naming the surfaces that
collide.

The two reserved names are held one step earlier, because a validator that ran only when
somebody chose to run it would be the weaker guard: moving the Swagger UI onto `/docs` while
the documentation still holds it is a mount collision the validator and the router both
refuse, and moving it there after removing the documentation fails
`the_reserved_mounts_belong_to_the_surfaces_that_own_them` in `http/surfaces.rs` and case
`89_web_surface`, which assert the mounts against the producers that must own them. The
names are enforced by tests that cannot be skipped, not by a runtime finding. `majordomus generate --check` fails when the committed
topology projection under `docs/generated/` is not what the registry now resolves, and
`scripts/generate-site-data --check` fails when the website's derived copy has drifted from
it — so a surface added without regenerating is a red build rather than a stale page.

# Verification

`bash test/run.sh 89_web_surface`, which proves from outside the executable that `/swagger`
serves the Swagger UI, that `/docs` serves documentation rather than the viewer, that a
built documentation mount does not swallow its neighbours, that no path traversal escapes
it, that the landing page's links and the machine-readable index describe the same set, and
that neither rendering of `/` prints where the checkout sits on the host.

`majordomus web validate`, run by `scripts/rust-check`, holds the topology's invariants; the
crate's own suites cover the model and the router, including the ordering that makes
ownership deterministic and the refusals at construction. `majordomus generate --check`
holds `docs/generated/web.json` current, and `scripts/generate-site-data --check` holds the
website's copy of it current, so a surface added without regenerating fails the build rather
than ageing quietly on a page.

The `site-basepath` gate in `.ai/repo/ci/gates.yaml` runs `scripts/site-basepath-check`,
which refuses an origin-absolute link in the documentation source — the one thing that would
make the published site and the `/docs` mount need different sources.

`/api/v1/web/surfaces` answers the same resolution to a program. Over a served socket it
answers what that process serves; from the command line it answers the repository's. Both
read one value resolved once per process, and there is no second discovery behind either.
