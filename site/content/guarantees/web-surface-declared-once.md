+++
title = "Every web surface is declared once at its producer and resolved into one topology, from which the router, the landing page and the machine-readable index are each projected"
description = "Everything this repository exposes over HTTP is one resolved object: an identity, a mount, a kind, a category, a visibility, the producer that made it, and the provenance of each of those values. The routes the executable answers itself come from the capability! declarations that already exist and are already projected to the CLI, MCP and OpenAPI. A generated directory — a test report, a benchmark report, the documentation build — declares itself beside its own output rather than in a list somewhere else. The application's site is read from the site generator's configuration. There is no central manifest of surfaces, and the resolved manifest written for diagnostics is disposable: nothing reads it as truth that could not be recomputed."
weight = 154
[extra]
claim_id = "web-surface-declared-once"
status = "guaranteed"
source = "docs/claims/web-surface-declared-once.md"
+++
{% raw %}

## What it means

Everything this repository exposes over HTTP is one resolved object: an identity, a mount, a kind, a category, a visibility, the producer that made it, and the provenance of each of those values. The routes the executable answers itself come from the `capability!` declarations that already exist and are already projected to the CLI, MCP and OpenAPI. A generated directory — a test report, a benchmark report, the documentation build — declares itself beside its own output rather than in a list somewhere else. The application's site is read from the site generator's configuration. There is no central manifest of surfaces, and the resolved manifest written for diagnostics is disposable: nothing reads it as truth that could not be recomputed.

Every consumer reads that one resolution. The router dispatches by asking which surface owns a request path instead of naming surfaces in match arms. The landing page at `/` is rendered from the topology, grouped by the category each surface declares. `/api/v1/web/surfaces` answers the same value to a program. The publication composes the surfaces that contribute files. This page and its tables are generated from the committed projection of it. A surface that appears in one of those and not another is the drift the rule forbids, and a route table written by hand — in Rust, in a template, in Markdown, in JSON, YAML or TOML — is a defect whether or not it currently agrees.

## How it works

`apps/majordomus-cli/src/web/model.rs` holds the vocabulary: `SurfaceKind` is behavioural (`StaticDirectory`, `NativeRoute`, `Redirect`) rather than nominal, so a test report and a benchmark report differ in data and not in type. `Mount` normalises and refuses rather than repairs — no `..`, no empty segment, no query, no backslash — and `Topology::new` orders surfaces by depth so route precedence is computed rather than left to whoever inserted last. `discover.rs` resolves from the three inference sources; `validate.rs` holds the invariants, checked per world so that a deployment surface and a runtime surface may share a mount they can never both answer on; `compose.rs` builds the publishable tree. `http/surfaces.rs` narrows the topology to the running process and binds a handler to each native surface, refusing at construction — not at request time — when two surfaces collide or a native surface has no code behind it. `Surface::served_by` drops a surface whose runtime feature this build or invocation lacks, so the registry describes the effective process rather than the maximum one.

`majordomus generate` writes `docs/generated/web.json` and `generate --check` holds it current. `scripts/generate-site-data` reads that committed document, refuses one that is not `majordomus/web-topology/v1` or that omits a required field, and projects it into `site/data/generated/web.json` and the page you are reading.

## How to see it

```bash
majordomus web list                     # the resolved topology, route-precedence order
majordomus web explain swagger          # where each of that surface's values came from
majordomus web validate                 # the invariants, per world
majordomus generate --check             # docs/generated/web.json is current
curl -s localhost:PORT/api/v1/web/surfaces | jq '.surfaces[].mount'
bash test/run.sh 89_web_surface         # the page and the index describe the same set
```

## What it does not cover

`built_from` — the revision a static artifact was built from — is deliberately absent from the committed projection. It is a fact of one checkout's artifacts, and committing it would make every clone report drift under `generate --check`. It is present on the live answer and in the `surface.json` a producer writes.

The rule constrains where a surface is declared and what may read it; it does not constrain what a surface serves. A producer remains free to write whatever it writes into its own directory.

## Why it exists

Two more surfaces arriving at once is what decided the shape. Added the way the earlier ones were, each would have cost a route in the router, an output path in a script, a path class in the gate model, a copy step in publication, a row in a documentation table and a mention in the deployment workflow — six registrations for one directory of HTML, and the seventh surface would cost the same again. The registration a person forgets is never the router's, because a broken route is noticed in a minute; it is the documentation's, and a route reference that lies is noticed only after it has been believed. ADR 0013 records the decision and what it rejected.
{% endraw %}
