<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `why` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.1 -->
# Module `why` — Why

The operational failure modes this tool is a response to: the moments a reader recognises, the audiences that recognise them, the areas they fall under, and what a reader's own symptoms imply. Every entry is a file under the layer's why section; nothing here holds a list, and a moment added there is answered by all of these without a registration anywhere.

Stability: behaviorally_verified. Capabilities: 6.

## `why.areas` — Operational areas

Every operational area the catalogue declares, each with the public moments that fall under it. Membership is derived from the moments and is never listed in an area's own file.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_why_areas` |
| MCP resource | `majordomus://why/areas` |
| HTTP | `GET /api/v1/why/areas` |
| CLI | `majordomus why areas` |
| cache | process, 4 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::why |
| tags | why, catalogue |

Input: none.

Output: `AreaList`.

## `why.audiences` — Audiences

Every audience the catalogue declares, each with the public moments that name it. Membership is derived from the moments and is never listed in an audience's own file.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_why_audiences` |
| MCP resource | `majordomus://why/audiences` |
| HTTP | `GET /api/v1/why/audiences` |
| CLI | `majordomus why audiences` |
| cache | process, 4 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::why |
| tags | why, catalogue |

Input: none.

Output: `AudienceList`.

## `why.diagnose` — Diagnose a selection

What a reader's own symptoms imply: the moments the selection resolves to, the operational areas and audiences they weigh towards, and the capabilities, commands, claims, rules and use cases that answer them — each carrying the moments that produced it. Counting, not inference: there is no weighting and no percentage.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_why_diagnose` |
| HTTP | `GET /api/v1/why/diagnose` |
| CLI | `majordomus why diagnose` |
| cache | process, 32 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::why |
| tags | why, diagnostics |

| input | type | required | description |
|---|---|---|---|
| `signals` | string | no | Signal ids or moment ids, separated by commas. Empty selects nothing and is
answered with an empty diagnosis rather than an error. |

Output: `Diagnosis`.

## `why.list` — The catalogue

Every operational moment this repository holds, narrowed by any of the facets the catalogue itself reports, with the audiences, the areas, the derived filters and the counts. The default is the public catalogue; pass status=any for the drafts too.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_why` |
| MCP resource | `majordomus://why` |
| HTTP | `GET /api/v1/why` |
| CLI | `majordomus why list` |
| cache | process, 32 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::why |
| tags | why, catalogue |

| input | type | required | description |
|---|---|---|---|
| `audience` | string or null | no | Only moments this audience recognises. |
| `area` | string or null | no | Only moments in this operational area. |
| `tag` | string or null | no | Only moments carrying this tag. |
| `severity` | string or null | no | Only moments of this severity. |
| `frequency` | string or null | no | Only moments of this frequency. |
| `lifecycle` | string or null | no | Only moments at this stage of work. |
| `capability` | string or null | no | Only moments naming this capability of the executable. |
| `command` | string or null | no | Only moments naming this command. |
| `featured` | boolean or null | no | Only moments the homepage features. |
| `status` | string or null | no | Only moments of this status. Absent means the public ones (`stable`); pass `any`
for everything the catalogue holds. |
| `q` | string or null | no | Case-insensitive text, matched against the identity, the titles, the hook, the
summary, the tags, the aliases, the signals, the examples and the body. |

Output: `CatalogueView`.

## `why.moment` — One moment

One operational moment in full: what it looks like, why it happens, what it costs, what this tool does about it, and every relation derived from its metadata — the responsibilities its claims belong to, the moments that name it, and the moments nearest it by shared area, audience and tag.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_why_moment` |
| HTTP | `GET /api/v1/why/moment` |
| CLI | `majordomus why show` |
| cache | process, 64 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::why |
| tags | why |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The moment's id, as `why.list` gives it. This is also its slug and its route. |

Output: `MomentDetail`.

## `why.validate` — Validate the catalogue

Every finding over the catalogue: a reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; and a public record that does not meet the floor its status promises. Errors make the catalogue invalid; warnings do not.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_why_validate` |
| HTTP | `GET /api/v1/why/validate` |
| CLI | `majordomus why validate` |
| cache | process, 2 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::why |
| tags | why, introspection |

Input: none.

Output: `ValidationReport`.

