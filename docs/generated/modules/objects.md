<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `objects`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `objects` — Objects

The declarative objects of the repository's AI layer: rules, prompts, profiles, policy, documents, milestones, issues, claims, and whatever kinds the repository adds; listed, read by URI, and searched.

Stability: behaviorally_verified. Capabilities: 3.

## `objects.get` — Get one object

One object by URI (majordomus://<kind>/<identity>): metadata, provenance and content.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_get` |
| HTTP | `GET /api/v1/object` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::objects |
| tags | objects |

| input | type | required | description |
|---|---|---|---|
| `uri` | string | yes | `majordomus://<kind>/<identity>`. |

Output: `ObjectView`.

## `objects.list` — List objects

List the declarative objects of the repository's AI layer, optionally by kind or tag.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_list` |
| HTTP | `GET /api/v1/objects` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::objects |
| tags | objects |

| input | type | required | description |
|---|---|---|---|
| `kind` | string or null | no | Only objects of this kind; the kinds present are listed by `repository.info`. A kind
the repository does not have is an invalid input, not an empty answer. |
| `tag` | string or null | no | Only objects whose metadata tags include this tag. |

Output: `ObjectList`.

## `objects.search` — Search objects

Case-insensitive substring search over identities, titles, descriptions and content.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_search` |
| HTTP | `GET /api/v1/search` |
| cache | process, 64 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::objects |
| tags | objects |

| input | type | required | description |
|---|---|---|---|
| `query` | string | yes | Case-insensitive substring, matched against identity, title, description and content. |
| `kind` | string or null | no | Only objects of this kind; a kind the repository does not have is an invalid input. |
| `limit` | integer or null | no | At most this many hits (default 20, at most 200). |

Output: `SearchResult`.

