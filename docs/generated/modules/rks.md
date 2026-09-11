<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `rks` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.2.0 -->
# Module `rks` — Repository knowledge

What the repository knows about itself: typed nodes with claims, evidence and relations, read off the tree by deterministic extractors and held against a committed baseline for freshness, conflicts, coverage and canonicality. One scan per process, shared; every capability here is a slice of it. Nothing here writes: the baseline is recorded and a reconciliation accepted from the command line.

Stability: implemented. Capabilities: 17.

## `rks.canonicality` — The canonicality audit

Every query and command with its one canonical source, the surfaces derived from it, the hand-written files that name it and the manual maintenance surface that follows; every orphan projection, undeclared generated file, missing artifact, suspected mirror and expired exception; the typed exceptions with their status; the verdict.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_canonicality` |
| HTTP | `GET /api/v1/canonicality` |
| CLI | `majordomus knowledge canonicality` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, canonicality, gate |

| input | type | required | description |
|---|---|---|---|
| `capability` | string or null | no | Only this capability's row, with every violation still listed. |

Output: `CanonicalityAudit`.

## `rks.check` — The check against the baseline

New debt the baseline does not tolerate, debt it tolerates, and debt it tolerates that is gone; the verdict in the policy's mode or the one given: observe and warn never fail, protect fails on new debt, strict fails on any. What `majordomus knowledge check` and the CI gate print.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_check` |
| HTTP | `GET /api/v1/knowledge/check` |
| CLI | `majordomus knowledge check` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, gate |

| input | type | required | description |
|---|---|---|---|
| `mode` | object | no | The mode to check in; the policy's when unset. |

Output: `KnowledgeCheckReport`.

## `rks.conflicts` — The conflicts

Every subject with two values for one functional predicate from two sources: both sides with their provenance and evidence, the severity, the basis, whether a person accepted it, and the remedy.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_conflicts` |
| HTTP | `GET /api/v1/knowledge/conflicts` |
| CLI | `majordomus knowledge conflicts` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge |

| input | type | required | description |
|---|---|---|---|
| `open_only` | boolean | no | Only open conflicts. |

Output: `KnowledgeConflictsReport`.

## `rks.context` — The context for some paths

What an agent should read before touching some paths: the nodes whose sources are under them and what those govern, describe and depend on one hop out, most governing first; the claims among them that are not current, as caveats; the open conflicts and the gaps; cut to a budget, and to public knowledge only when asked.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_context` |
| HTTP | `GET /api/v1/knowledge/context` |
| CLI | `majordomus knowledge context` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, context |

| input | type | required | description |
|---|---|---|---|
| `paths` | array | no | The paths about to be touched; the whole repository when empty. |
| `budget` | integer | no | The budget in bytes of rendered JSON; the default when `0`. |
| `public_only` | boolean | no | Only public knowledge: what may leave the machine. |

Output: `KnowledgeContextBundle`.

## `rks.coverage` — Coverage

Over denominators the executable defines deterministically — components, capabilities, references, curated records, generated artifacts, layer objects — how many are covered and which are missing. Numbers, never percentages.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_coverage` |
| HTTP | `GET /api/v1/knowledge/coverage` |
| CLI | `majordomus knowledge coverage` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, coverage |

Input: none.

Output: `KnowledgeCoverageReport`.

## `rks.explain` — Why the model says what it says

One node explained: how it is known (observed, declared, curated, derived), what evidence it rests on with fingerprints, every claim with its freshness and the reason, what it relates to, the conflicts it is party to, the gaps about it, and what to do. For a capability, the same answer names its canonical source and every derived surface.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_explain` |
| HTTP | `GET /api/v1/knowledge/explain` |
| CLI | `majordomus knowledge explain` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, why |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The node id (`component:majordomus-cli`, `rule:project.alpha@1`), a capability id
(`objects.get`), or an object URI (`majordomus://rule/project.alpha@1`). |

Output: `KnowledgeExplanation`.

## `rks.extractors` — How the model is made

Every extractor with the kinds, relations and predicates it declares; the node kind vocabulary with who owns each kind; the semantic providers this executable ships and whether each is remote; the schema versions read and written and the migrations known.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_extractors` |
| HTTP | `GET /api/v1/knowledge/extractors` |
| CLI | `majordomus knowledge extractors` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, introspection |

Input: none.

Output: `KnowledgeExtractorsReport`.

## `rks.gaps` — The gaps

Everything the repository could know and does not: undocumented components, unresolved references, unverified curated knowledge, unexercised capabilities, canonicality violations — each with the reason and the remedy, optionally one category.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_gaps` |
| HTTP | `GET /api/v1/knowledge/gaps` |
| CLI | `majordomus knowledge gaps` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge |

| input | type | required | description |
|---|---|---|---|
| `category` | object | no | Only gaps of this category. |

Output: `KnowledgeGapsReport`.

## `rks.get` — One knowledge node

One node with everything that bears on it: its claims with their provenance, evidence, verification and freshness; its evidence resolved with fingerprints; the relations in and out; the conflicts it is the subject of; the gaps about it. The id may be a node id, a capability id, an object URI or a path.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_get` |
| HTTP | `GET /api/v1/knowledge/node` |
| CLI | `majordomus knowledge show` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The node id (`component:majordomus-cli`, `rule:project.alpha@1`), a capability id
(`objects.get`), or an object URI (`majordomus://rule/project.alpha@1`). |

Output: `KnowledgeNodeView`.

## `rks.graph` — A slice of the knowledge graph

Nodes and typed relations: around one root to a depth, or every node of one kind, capped. Each node carries its freshness so that a drawing can colour what is stale.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_graph` |
| HTTP | `GET /api/v1/knowledge/graph` |
| CLI | `majordomus knowledge graph` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, graph |

| input | type | required | description |
|---|---|---|---|
| `root` | string or null | no | Cut the slice around this node. |
| `depth` | integer | no | How many hops from the root; `2` when unset. |
| `kind` | string or null | no | Without a root: only nodes of this kind. |
| `limit` | integer | no | At most this many nodes; the executable's cap applies. |

Output: `KnowledgeGraphSlice`.

## `rks.impact` — What a change set touches

The working tree against a base, two revisions, or named paths: the entries that changed where an extractor can tell, the nodes whose evidence changed, the claims resting on it, the nodes reached along propagating relations with the hop count, which extractors a scan would re-run, and the changed paths nothing in the model knows about.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_impact` |
| HTTP | `GET /api/v1/knowledge/impact` |
| CLI | `majordomus knowledge impact` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, change |

| input | type | required | description |
|---|---|---|---|
| `base` | string or null | no | The base to compare with; `HEAD` when unset. |
| `to` | string or null | no | Compare `base` with this revision instead of with the working tree. |
| `paths` | array | no | Name the changed paths outright instead of asking git. |

Output: `KnowledgeImpactReport`.

## `rks.inspect` — Inspect a change set

What a change set means for the knowledge before it is merged: the paths that changed against a base, the nodes and claims they touch, every capability the change adds with the checklist of surfaces derived for it, and the debt it introduces — freshness the baseline does not tolerate and canonicality violations that count. The pull-request gate.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_change_inspect` |
| HTTP | `GET /api/v1/knowledge/inspect` |
| CLI | `majordomus knowledge inspect` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, change, gate |

| input | type | required | description |
|---|---|---|---|
| `base` | string or null | no | The base to compare the working tree with; `HEAD` when unset. |

Output: `KnowledgeInspection`.

## `rks.list` — List knowledge nodes

The nodes a filter matches — by kind, provenance, freshness, ownership, visibility, extractor, or a substring of id or title; optionally only debt — one page at a time, each with its freshness and the reason it is not current.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_list` |
| HTTP | `GET /api/v1/knowledge/nodes` |
| CLI | `majordomus knowledge list` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge |

| input | type | required | description |
|---|---|---|---|
| `kind` | string or null | no | One node kind. |
| `provenance` | object | no | One provenance. |
| `freshness` | object | no | One freshness. |
| `ownership` | object | no | One ownership. |
| `visibility` | object | no | One visibility. |
| `extractor` | string or null | no | One extractor. |
| `query` | string or null | no | A substring of the id or the title, case-insensitive. |
| `debt` | boolean | no | Only nodes whose freshness is debt. |
| `offset` | integer | no | Skip this many matches. |
| `limit` | integer | no | Answer at most this many; `0` means the default. |

Output: `KnowledgePage`.

## `rks.model` — The whole model

The knowledge model as one document, `majordomus/knowledge/v1`: every extractor, evidence, node with claims, relation, conflict, gap and coverage row, with the fingerprint. Optionally the public projection only, which is what leaves the repository.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_model` |
| MCP resource | `majordomus://knowledge/model` |
| HTTP | `GET /api/v1/knowledge/model` |
| CLI | `majordomus knowledge scan` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, export |

| input | type | required | description |
|---|---|---|---|
| `public` | boolean | no | Only the public projection: what may leave the repository. |

Output: `KnowledgeModel`.

## `rks.proposals` — The reconciliation proposals

What to do about every open conflict, stale or unverified claim, unresolved reference and gap, as proposals with an owner: which a person edits, and which `majordomus knowledge reconcile --accept` applies by recording a verification in the baseline.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_proposals` |
| HTTP | `GET /api/v1/knowledge/proposals` |
| CLI | `majordomus knowledge reconcile` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge |

Input: none.

Output: `KnowledgeReconciliation`.

## `rks.search` — Search the knowledge

Ids and titles first, then summaries, then claim values; ranked by where the query matched and then by id, so that two runs agree.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge_search` |
| HTTP | `GET /api/v1/knowledge/search` |
| CLI | `majordomus knowledge search` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, search |

| input | type | required | description |
|---|---|---|---|
| `query` | string | yes | What to look for, case-insensitive. |
| `limit` | integer | no | At most this many hits; `0` for the default. |

Output: `KnowledgeSearchResult`.

## `rks.status` — The knowledge status

One scan, summarised: the repository and the reference it is judged against, nodes by kind, freshness and provenance, open conflicts, gaps by category, every coverage row, the check against the baseline in the policy's mode, the canonicality audit's verdict and the manual maintenance surface, and which extractors ran.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_knowledge` |
| MCP resource | `majordomus://knowledge` |
| HTTP | `GET /api/v1/knowledge` |
| CLI | `majordomus knowledge status` |
| cache | process, 32 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge |
| tags | knowledge, introspection |

Input: none.

Output: `KnowledgeStatusReport`.

