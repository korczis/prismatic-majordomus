+++
title = "The web surface"
description = "the web surface: every surface discovered from its producer rather than registered, the two reserved namespaces (`/docs` is documentation, `/swagger` is Swagger UI), what a surface declares, how to add one, how the documentation is built for its mount and served safely, and what is enforced where"
weight = 43
[extra]
source = "docs/WEB.md"
+++

{% raw %}

`majordomus serve` and `majordomus mcp` bind one shared server per repository. What that
server serves is not written down anywhere as a list. It is **discovered**: from the
executable's own declarations, from the site's configuration, and from a `surface.json` a
producer writes beside its output. One resolution then feeds the router, the home page, the
machine-readable index, the validator, the publication and the generated reference.

The decision is
[ADR 13](../.ai/repo/adrs/0013-every-web-surface-is-discovered-from-its-producer-resolved-o.md);
the rule that owns the invariant is `project.web-surface-declared-once`. Behaviour as
implemented and tested; where this document and the executable disagree, the document is
wrong and changes in the same commit.

<pre class="mermaid">
flowchart TD
  subgraph producers["PRODUCERS"]
    decl["capability! declarations"]
    config["site/config.toml"]
    surfacejson["target/web/&amp;lt;id&amp;gt;/surface.json"]
  end
  resolution["ONE RESOLUTION&lt;br&gt;web::discover → Topology,&lt;br&gt;validated, narrowed per process"]
  subgraph projections["DERIVED PROJECTIONS"]
    router["the HTTP router"]
    home["the home page at /"]
    api["/api/v1/web/surfaces"]
    webjson["docs/generated/web.json →&lt;br&gt;the website's route reference"]
    cli["majordomus web list / explain /&lt;br&gt;validate / compose"]
    bench["the benchmark targets"]
    log["the startup log"]
  end
  producers --&gt; resolution
  resolution --&gt; projections
</pre>


## The effective surface

Every path below is the mount its surface declares. Nothing in this table is typed twice:
`majordomus web list` prints it from the same resolution, and a mount that moves moves here.

<div class="overflow-x-auto" tabindex="0">

| Path | Surface | What it is | Kind |
|---|---|---|---|
| `/` | `home` | This process, and everything it serves | route |
| `/docs/` | `docs` | This repository's documentation | directory |
| `/swagger` | `swagger` | Swagger UI over the OpenAPI document | route |
| `/openapi.json` | `openapi` | The OpenAPI document, generated from the registry | route |
| `/api/v1/` | `api` | The capability routes | route |
| `/cockpit` | `cockpit` | The registry, rendered for a person | route |
| `/mcp` | `mcp` | MCP over HTTP, when this process serves a shared server | route |

</div>


Two names are reserved and may never be repurposed:

- **`/docs` is documentation.** It used to be Swagger UI, which is how one name came to
  mean two things in four files. It does not any more.
- **`/swagger` is Swagger UI.** It is a viewer for the API, and a viewer for the API is not
  the documentation.

The reservations are data — `web::discover::reserved()`, each path the same constant its
surface is declared with — and everything that cares reads them. `majordomus web validate`
refuses a topology in which a reserved mount is held by anything but its owner, or in which
an owner has wandered off its mount (`surface.reserved-namespace`); the router refuses to be
built from such a topology, before it answers a request; and `docs/generated/web.json`
publishes the map, which is what the website's reserved-names table renders. A collision
would catch the two names taking each other's mount only while both surfaces exist —
this catches it when one of them is gone, which is the state the repository was in.

Reservations are about what a *process serves*. A publication's `/` belongs to the site as
it is deployed, and always has.

## What a surface is

```rust
Surface {
    id,            // identity and selector; unique across the topology
    title,         // one line, shown on the home page and in a listing
    category,      // interface | documentation | api | protocol | report
    visibility,    // public (offered to a person) | internal (served, not advertised)
    kind,          // static (a generated directory) | native (a route) | redirect
    mount,         // where it answers; it owns this path and everything under it
    producer,      // what writes it; the home page names this when it has not run
    feature,       // the runtime capability it needs: mcp | cockpit | none
    artifact,      // the generated directory, repository-relative, for a static surface
    index,         // the file the mount itself answers with
    availability,  // both | served-only | published-only
    built_from,    // the revision its producer recorded, when it recorded one
    provenance,    // per field: where the resolved value came from
}
```

Two properties do most of the work.

**`availability` is two worlds, not one flag.** What a running process serves and what a
publication contains are different sets. The site as it is deployed owns `/` of a GitHub
Pages origin and is never served by this executable; the home page owns `/` of a process
and is never published. They share a mount and are not in conflict, because they never
meet — and the validator checks mount ownership per world for exactly that reason.

**`feature` describes the effective binary.** A process that answers no MCP has no MCP
surface, so the home page cannot link to one and the startup log cannot name one. The
narrowing is a filter over the resolved value, never a second discovery.

## What every surface is rendered with

Whatever a surface is — the published site, the Cockpit, a report the executable renders,
the Swagger shell — it is rendered with one design: the roles, status vocabulary, type
scale, theme contract and brand declared once in `share/design/tokens.yaml` and projected
by `majordomus generate design` into the sheets each surface loads. A served page carries
the declaration's fingerprint (`--mj-design` in its stylesheet, `data-design` on the page)
and the executable answers the same fingerprint from `GET /api/v1/design`, so a stale bundle
is visible on the page. The pipeline, the extension flow and the gates are
[`DESIGN_SYSTEM.md`](@/docs/design-system.md).

## Adding a surface

**A generated report** — no code at all. Write your output into `target/web/<id>/` and a
`surface.json` beside it:

```json
{
  "schema": "web-surface/v1",
  "id": "coverage",
  "mount": "/coverage",
  "title": "Line coverage of the crate",
  "producer": "just coverage"
}
```

It is now discovered, served under `/coverage/`, listed on the home page, present in
`/api/v1/web/surfaces`, validated for collisions, composed into the publication and named
in the generated reference. There is no registry to add it to, because there is none.

**A route the executable answers** — one declaration and one handler. Add the `Surface` to
`web::discover::native_all()` with its mount, category and visibility, and an arm to
`http::surfaces::Native::of` binding it to code. Those two are the whole change: a native
surface declared with no handler refuses the router at construction, and the test
`every_native_surface_has_a_handler` holds the two halves in step. Everything downstream —
router, home page, route index, startup log, generated reference — follows.

**A capability** is not a surface. It is a `capability!` block with an `http` exposure, and
it appears under `/api/v1/` because the `api` surface owns that prefix
([`CAPABILITIES.md`](@/docs/capabilities.md)).

## How `/docs` is built and served

The documentation under `/docs/` is the **same Zola source** GitHub Pages renders. There is
no second copy and no second generator; the two builds differ in one argument.

<pre class="mermaid">
flowchart LR
  src["site/"]
  src --&gt;|"scripts/site-build"| public["site/public/&lt;br&gt;base_url https://majordomus.dev"]
  src --&gt;|"scripts/site-build --serve"| served["target/web/docs/&lt;br&gt;base_url /docs"]
</pre>


`zola build --base-url /docs` emits root-relative links (`/docs/commands/`), so the mount is
correct behind any host, port or reverse-proxy prefix — no hostname is ever baked into the
build. `--serve` also writes the `surface.json` that makes the directory discoverable, with
the revision it was built from.

`majordomus serve` serves an already-built tree and never invokes Zola: a build per request
is not a thing this executable does, and Zola is an external tool that may not be installed.
A request under `/docs/` when the tree is absent answers `503` naming the directory that is
missing and the command that writes it, rather than a 404 that explains nothing. The home
page shows the same surface as *not built* with that command beside it, instead of offering
a link that is certain to fail.

The generated tree lives under `target/web/`, which is ignored: it is derived state with the
same status as every other generated tree here — reproducible, never read as source, and a
stale artifact is a `majordomus web validate --artifacts` finding rather than a diff.

## Static file serving

`/docs/**` is served by `web::files`. A request path is decomposed into segments and every
segment checked against a conservative character set before the filesystem is touched; a
segment that is empty, `.` or `..` refuses the request. The resolved path is canonicalised
and required to still be inside the canonical root, which closes the door a symlink inside
the directory would otherwise open. There is no concatenation of untrusted text anywhere in
that file, and a file whose extension is not one the surface generates is not served.

A file is read once and answered from memory afterwards, up to a ceiling; documents carry
`Cache-Control: no-cache`, because the pages are rebuilt from a working tree somebody is
editing and a page one build behind is worse than a request.

## Reading the topology

```sh
majordomus web list                  # every surface: id, kind, mount, category, world, source
majordomus web explain docs          # one surface, and where each of its values came from
majordomus web validate              # the invariants; exit 10 with every finding and its remedy
majordomus web validate --artifacts  # and require every producer to have run
majordomus web compose               # every published surface into one tree
majordomus web list --format json    # the same value as JSON
```

Over HTTP and MCP the same resolution is `/api/v1/web/surfaces`, the tool
`majordomus_web_surfaces` and the resource `majordomus://web`. One nuance: over a served
socket it answers the **narrowed** topology, because the router hands its capability calls a
context whose topology is what that process serves; from the command line or a standalone
MCP session it answers the repository's. Both read one value, resolved once for each
generation of the repository a long-lived process reads (`crate::live`), never per request.

`docs/generated/web.json` is the committed projection, written by `majordomus generate` and
checked by `generate --check`. It exists because the site generator runs without a Rust
toolchain and reads committed artifacts; the Rust resolution is authoritative and the file
is never edited by hand. It omits `built_from`, which is a fact of one checkout's artifacts
and would make every clone report drift.

## What is enforced, and where

<div class="overflow-x-auto" tabindex="0">

| Invariant | Enforced by |
|---|---|
| Ids unique; one owner per path in each world; no nested mount | `majordomus web validate`, in `scripts/rust-check` |
| A native surface has a handler | the router refuses to build; `every_native_surface_has_a_handler` |
| `/docs` is documentation, `/swagger` is Swagger UI | `project.web-surface-declared-once`, `test/cases/89_web_surface.sh` |
| The home page covers every public served surface | `web::home` tests, and case 89 over a real socket |
| `docs/generated/web.json` current | `majordomus generate --check` |
| No origin-absolute link forces the two builds apart | `scripts/site-basepath-check` |
| The new routes are timed | `SystemTarget::HttpHome`, `HttpSwagger`, `HttpIndex` |

</div>

{% endraw %}
