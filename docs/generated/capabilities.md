<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Capability reference

Every capability this executable ships, as the registry holds it. MCP tools and resources, HTTP routes, the OpenAPI document (`openapi.json` beside this file, and `/openapi.json` when serving), Swagger UI and the command line's `capabilities` commands are projections of the same entries; nothing below is declared anywhere else.

## Executable capabilities

| id | kind | stability | MCP tool | MCP resource | HTTP | CLI | provenance |
|---|---|---|---|---|---|---|---|
| `capabilities.describe` | query | behaviorally_verified | `majordomus_capability` | — | `GET /api/v1/capability` | `majordomus capabilities describe` | builtin majordomus_cli::capability::builtin::capabilities |
| `capabilities.list` | query | behaviorally_verified | `majordomus_capabilities` | — | `GET /api/v1/capabilities` | `majordomus capabilities list` | builtin majordomus_cli::capability::builtin::capabilities |
| `objects.get` | query | behaviorally_verified | `majordomus_get` | — | `GET /api/v1/object` | — | builtin majordomus_cli::capability::builtin::objects |
| `objects.list` | query | behaviorally_verified | `majordomus_list` | — | `GET /api/v1/objects` | — | builtin majordomus_cli::capability::builtin::objects |
| `objects.search` | query | behaviorally_verified | `majordomus_search` | — | `GET /api/v1/search` | — | builtin majordomus_cli::capability::builtin::objects |
| `peers.announce` | command | behaviorally_verified | `majordomus_announce` | — | `POST /api/v1/peers/announce` | — | builtin majordomus_cli::capability::builtin::peers |
| `peers.list` | query | behaviorally_verified | `majordomus_peers` | — | `GET /api/v1/peers` | — | builtin majordomus_cli::capability::builtin::peers |
| `repository.info` | query | behaviorally_verified | `majordomus_repository` | `majordomus://repository` | `GET /api/v1/repository` | — | builtin majordomus_cli::capability::builtin::repository |

### `capabilities.describe` — Describe one capability

One capability by canonical id: its kind, schemas, provenance, stability, exposures, benchmark and cache policy.

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The canonical id, e.g. `repository.info` or `rule.majordomus.scope-integrity@1`. |

Output: `Capability`.

### `capabilities.list` — List capabilities

Every capability of this executable and this repository, with its kind, stability, provenance and the projections it declares.

| input | type | required | description |
|---|---|---|---|
| `kind` | string or null | no | Only capabilities of this kind: `query`, `command` or `resource`. |
| `exposure` | string or null | no | Only capabilities exposed through this projection: `mcp`, `http` or `cli`. |

Output: `CapabilityList`.

### `objects.get` — Get one object

One object by URI (majordomus://<kind>/<identity>): metadata, provenance and content.

| input | type | required | description |
|---|---|---|---|
| `uri` | string | yes | `majordomus://<kind>/<identity>`. |

Output: `ObjectView`.

### `objects.list` — List objects

List the declarative objects of the repository's AI layer, optionally by kind or tag.

| input | type | required | description |
|---|---|---|---|
| `kind` | string or null | no | Only objects of this kind; the kinds present are listed by `repository.info`. A kind
the repository does not have is an invalid input, not an empty answer. |
| `tag` | string or null | no | Only objects whose metadata tags include this tag. |

Output: `ObjectList`.

### `objects.search` — Search objects

Case-insensitive substring search over identities, titles, descriptions and content.

| input | type | required | description |
|---|---|---|---|
| `query` | string | yes | Case-insensitive substring, matched against identity, title, description and content. |
| `kind` | string or null | no | Only objects of this kind; a kind the repository does not have is an invalid input. |
| `limit` | integer or null | no | At most this many hits (default 20, at most 200). |

Output: `SearchResult`.

### `peers.announce` — Announce what this peer is working on

Tell the other peers of this shared server what the calling session is doing and which paths it expects to touch. Changes this process's memory only; the repository is never written. Needs an MCP session: over plain HTTP there is no caller.

| input | type | required | description |
|---|---|---|---|
| `intent` | string | yes | One line, in the peer's words: the task, the question, the intent. |
| `scope` | array | no | Repository-relative paths the peer expects to touch. Informational: other peers
read it to avoid a collision; nothing here enforces it. |

Output: `Peer`.

### `peers.list` — List peers

Every client attached to this shared server: id, the client's own name and version from its initialize, transport, when it attached, when it was last seen, and what it announced. In-memory, gone with the process.

Input: none.

Output: `PeerList`.

### `repository.info` — Repository and index state

The repository root, layer sections, git state, discovery mode, kinds present, every diagnostic, and the capability registry counted.

Input: none.

Output: `RepositoryReport`.

## Declarative resources

Every object of the repository's AI layer is a capability of kind `resource` with the id `<kind>.<identity>` (`rule.majordomus.scope-integrity@1`, `prompt.continue`, `document.docs/CLI.md`), exposed as the MCP resource `majordomus://<kind>/<identity>` and read over HTTP through `objects.get`. They are not listed here: they are the repository's, not the executable's, and `majordomus capabilities list --kind resource` answers for the repository at hand.

## Infrastructure routes

The HTTP projection's own routes, not capabilities: `/`, `/openapi.json`, `/docs`, `/mcp`. `/docs` is a Swagger UI shell that loads `/openapi.json`; it embeds no specification.
