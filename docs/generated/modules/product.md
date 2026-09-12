<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `product` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `product` — Product

What this repository's product does for a person, as the features under the layer's features section declare it: each feature made of modules, commands, kinds, rules, documents, decisions, claims, use cases, Cockpit areas and web surfaces it names, with the interfaces it is exposed through, every count, the moments it answers and what is guaranteed derived from those references. The matrix of features against interfaces, the providers the tool has an adapter for, and the model's own validation. The homepage is a reader of this module and holds no inventory of its own.

Stability: behaviorally_verified. Capabilities: 5.

## `product.feature` — One feature

One product feature in full: the record as its file declares it, and everything derived from what it names — the capabilities of its modules with their tools, routes and command-line paths, the commands with their summaries, the objects of its kinds counted, the rules with their class and whether the tool enforces them, the documents, the decisions, the claims with their status, the use cases, the Cockpit areas and web surfaces with their routes, the moments it answers, and the interfaces all of that adds up to.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_feature` |
| HTTP | `GET /api/v1/product/feature` |
| CLI | `majordomus product show` |
| cache | process, 64 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::product |
| tags | product, features |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The feature's id, as `product.features` gives it. This is also its slug and its route. |

Output: `ResolvedRefs`.

## `product.features` — The features

Every product feature this repository declares, narrowed by any of the facets the model derives — featured, area, module, command, surface, text — with the interfaces each is exposed through, the counts behind it and what is guaranteed about it, none of which its file states. The default is the stable set; pass status=any for the drafts too.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_features` |
| MCP resource | `majordomus://product` |
| HTTP | `GET /api/v1/product/features` |
| CLI | `majordomus product list` |
| cache | process, 32 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::product |
| tags | product, features, introspection |

| input | type | required | description |
|---|---|---|---|
| `featured` | boolean or null | no | Only the features the homepage shows. |
| `status` | string or null | no | Only features of this status. Absent means the stable ones; pass `any` for
everything the model holds. |
| `area` | string or null | no | Only features serving this operational area. |
| `module` | string or null | no | Only features made of this capability module. |
| `command` | string or null | no | Only features made of this shell command. |
| `surface` | string or null | no | Only features exposed through this surface: `cli`, `api`, `mcp`, `cockpit`, `docs`. |
| `q` | string or null | no | Case-insensitive text, matched against the identity, the titles, the headline, the
summary, the tags and the body. |

Output: `FeatureList`.

## `product.matrix` — Features against interfaces

Every feature against the command line, the HTTP API, MCP, the Cockpit and the documentation, each mark derived from what the feature names; then every builtin module of the executable, every public command of the shell tool and every kind of the layer with the stable features that name it. A row with no feature is reported as a gap rather than hidden.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_product_matrix` |
| MCP resource | `majordomus://product/matrix` |
| HTTP | `GET /api/v1/product/matrix` |
| CLI | `majordomus product matrix` |
| cache | process, 2 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::product |
| tags | product, features, coverage |

Input: none.

Output: `Matrix`.

## `product.providers` — The providers

Every provider the tool has an adapter for — one per template the distribution ships — with the bootstraps this repository's policy renders through it, the client configuration it carries for the shared MCP server, and the hooks the policy wires. The set is the templates; nothing here is a list of vendors.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_providers` |
| MCP resource | `majordomus://product/providers` |
| HTTP | `GET /api/v1/product/providers` |
| CLI | `majordomus product providers` |
| cache | process, 2 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::product |
| tags | product, providers |

Input: none.

Output: `ProviderList`.

## `product.validate` — Validate the model

Every finding over the product model: a reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; a draft that is featured; a stable feature under its floors; and every module, command or kind that no stable feature names. Errors make the model invalid; warnings do not.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_product_validate` |
| HTTP | `GET /api/v1/product/validate` |
| CLI | `majordomus product validate` |
| cache | process, 2 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::product |
| tags | product, introspection |

Input: none.

Output: `ProductValidationReport`.

