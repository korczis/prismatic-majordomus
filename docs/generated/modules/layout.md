<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `layout` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.7 -->
# Module `layout` — Layer layout

The boundary between the layer's tracked and local halves, checked against the layouts their contracts declare: what lives where, and what must never appear there (I1102).

Stability: behaviorally_verified. Capabilities: 1.

## `layout.check` — Refuse a misplaced artifact

Every path a declared layout excludes, named with the contract it violates: a tracked file under the tracked half that git's ignore rules match (state that does not travel, under a contract that excludes cache or runtime), and a file under the local half that is a byte copy of a tracked canonical file or declares the schema of a kind share/kinds.yaml calls canonical (under the manifest's `local.excludes: [canonical]`). Only what a contract declares is judged. Reads git and the local half's files; no build and no network.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_layout_check` |
| HTTP | `GET /api/v1/layout/check` |
| CLI | `majordomus layout check` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::layout |
| tags | layout, governance, two-folder |

Input: none.

Output: `LayoutReport`.

