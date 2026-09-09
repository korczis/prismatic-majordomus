<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `objects` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.4.0 -->
# Module `objects` — Objects

The declarative objects of the repository's AI layer: rules, prompts, profiles, policy, documents, milestones, issues, claims, and whatever kinds the repository adds; listed, read by URI, and searched.

Stability: behaviorally_verified. Capabilities: 4.

## `objects.get` — Get one object

One object by URI (majordomus://<kind>/<identity>): metadata, provenance and content; a URI a query projects (majordomus://repository) answers that query as a JSON document. The same resolution serves the MCP resource read.

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
| `uri` | string | yes | `majordomus://<kind>/<identity>`, or a URI a query projects
(`majordomus://repository`). |

Output: `ResourceView`.

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

## `objects.verify` — Verify the index against the working tree

Read every file the layer was built from and compare it with what this process is serving. The index is built once at start-up and kept, which is what makes every other request cost nothing and what makes a file edited afterwards be served as it was; this is how a running server says whether that has happened, without being restarted to find out. A file that is one object is compared byte for byte; a collection file, whose objects the index keeps as members rather than as text, is compared by size, and every finding says which comparison was made. It reads every file of the layer, so it reports its progress file by file and stops when it is asked to.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_verify_objects` |
| HTTP | `GET /api/v1/objects/verify` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::objects |
| tags | objects, diagnostic |

| input | type | required | description |
|---|---|---|---|
| `kind` | string or null | no | Only objects of this kind. |
| `limit` | integer or null | no | How many objects to read at most; every one of them when absent. |

Output: `VerifyReport`.

