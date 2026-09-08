<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `web` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.1 -->
# Module `web` — Web surfaces

What this repository exposes over HTTP, resolved from the producers that make it rather than from a register anybody maintains: the routes the executable answers itself, the documentation build, and every generated report that declared its own mount. The same resolution serves the router, renders the home page and composes a publication.

Stability: behaviorally_verified. Capabilities: 1.

## `web.surfaces` — Every web surface, resolved

The web topology in route-precedence order, with each surface's mount, category, visibility, kind, producer, artifact, runtime feature and the provenance of every value a reader could be surprised by; and which ids are served, published and offered to a person. Answered from the resolution this process serves from, so it cannot disagree with what the router routes or what the home page lists.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_web_surfaces` |
| MCP resource | `majordomus://web` |
| HTTP | `GET /api/v1/web/surfaces` |
| cache | process, 2 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::web |
| tags | web, introspection |

Input: none.

Output: `SurfaceReport`.

