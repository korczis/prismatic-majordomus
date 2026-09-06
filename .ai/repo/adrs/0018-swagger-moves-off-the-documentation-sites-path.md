---
schema: adr/v1
id: adr-0018
kind: adr
title: Swagger UI moves to /swagger, because /docs belongs to the documentation site
status: accepted
date: 2026-09-06
tags: [web, routes, projections]
related:
  - rule:project.web-surface-topology
  - rule:project.ui-conformance
  - file:apps/majordomus-cli/src/http/swagger.rs
  - file:apps/majordomus-cli/src/web/validate.rs
  - test:test/cases/84_web_surfaces.sh
provenance:
  origin: authored
---

# 18. Swagger UI moves to /swagger, because /docs belongs to the documentation site

## Context

The running executable answered Swagger UI at `/docs`. That was uncontested while the
executable served only itself. Once the site became a web surface of the same topology,
mounted at the root, `/docs/**` was both the documentation section of the site — 52 pages —
and a route the executable answered itself.

The topology validated cleanly, because the nesting rule exempts the root application:
answering what nothing else claims is its whole job. The exemption was right and blind. A
native route mounted at a path the application has *files* under takes every page beneath
it, and the topology has no way to know that from mounts alone.

The UI conformance audit found it from the outside: 165 findings, every one a page under
`/docs` answering 404 to anybody using `majordomus serve` rather than the published site.
The published site was unaffected, which is why nobody had noticed.

## Decision

Swagger UI is served at `/swagger`. `/docs/**` belongs to the documentation site.

The route was declared once — `http::swagger::DOCS_PATH` — and named again in fourteen
places: the router's match arm, the OpenAPI document's list of infrastructure routes, the
`about` paragraphs every projection reads, the generated capability reference, two benchmark
targets, the shared server's log line, two "already running" messages, the MCP initialize
instructions, three Cockpit links and a route table. Every one of them now reads the
constant, so the move itself was one line.

`web validate` gains `surface.shadows-application`: a native route mounted where the root
application has pages is reported, with the count of pages it takes. A warning rather than
an error, because which of the two should move is intent — a person decides whether the
route or the section is renamed — and refusing to serve until they decide would be worse
than telling them.

The root is negotiated rather than claimed. A browser asking for `/` gets the application's
landing page; a client that did not ask for HTML gets the JSON route index `GET /` has
always answered. Everything below the root goes to the surface that owns it without a
question.

## Consequences

A link to the old Swagger route now reaches the documentation site's index instead of
Swagger UI. That is a break, and it is the better of the two: the site's `/docs/` is what
someone following a stale link was most likely looking for, and no redirect can serve both
`/docs` and `/docs/**` from one prefix without re-creating the collision this removes.

The published GitHub Pages site is unchanged. It never served Swagger; it serves the API
reference derived from the same OpenAPI document, at `/docs/api/`.

`/openapi.json` remains claimed by both the executable and the site, and `web validate` says
so. That overlap is deliberate — the published site has no executable and must carry the
document as a file — and the two serve the same bytes, which the derive check already
enforces. The warning is left standing rather than special-cased, because a validator that
knows about one blessed exception is a validator nobody trusts about the next one.

## Alternatives rejected

**Move the documentation site instead.** `/docs` is the conventional path for documentation
and the published site's own URL. Moving it would break every external link to the project's
documentation to keep a route only a developer with a running server ever visits.

**Serve Swagger at `/docs` and the site at `/docs/`.** One character apart, decided by a
trailing slash, in a router where every other rule is prefix ownership. It would work and
nobody could reason about it.

**A redirect from `/docs` to `/swagger`.** A redirect at `/docs` is the same collision: the
site's own `/docs/` index would have to answer through it. A redirect is what the topology
has `SurfaceKind::Redirect` for, and this is not the case for one.
