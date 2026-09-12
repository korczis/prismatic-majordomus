<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `knowledge_base` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `knowledge_base` — Knowledge base

What the knowledge deriver left for review and whether it is still writing: the candidate records awaiting promotion with the branch of the episode each came from, one record by id with every reference it names resolved against the index, the ledger and git, and the derivation status of this checkout judged against the policy's freshness thresholds. Read from the index and the ledger; written by nothing here — the deriver is the shell tool's, and a person promotes.

Stability: behaviorally_verified. Capabilities: 3.

## `knowledge_base.candidates` — Candidates awaiting review

Every knowledge record with status candidate under the candidates source class: its class, its assertion, the episode it came from and that episode's branch, and how long it has waited — measured from the derivation that wrote it, then the commit that added it, then its own date — judged against session.freshness; against the policy's cap. A candidate whose episode this checkout does not know is reported with no branch rather than hidden. Ordered by id.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_knowledge_candidates` |
| MCP resource | `majordomus://knowledge-candidates` |
| HTTP | `GET /api/v1/knowledge/candidates` |
| CLI | `majordomus knowledge candidates` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge_base |
| tags | knowledge, continuity |

Input: none.

Output: `KnowledgeCandidates`.

## `knowledge_base.record` — One knowledge record

One knowledge record by id — candidate or curated — with every reference it names resolved: a session against the tracked session records and the ledger, a task or decision against the ledger and the task store, a commit against git, a file or test against the index and the tree, a rule, an ADR, an issue or another record against the index. Which references dangle is the question a reviewer asks before promoting, and the one the integrity validator asks of every record. A `task:none` or `decision:none` reference is refused by name.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_knowledge_record` |
| HTTP | `GET /api/v1/knowledge/record` |
| CLI | `majordomus knowledge record` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge_base |
| tags | knowledge |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The record id, as the file name and `majordomus://knowledge/<id>` carry it. |

Output: `KnowledgeRecord`.

## `knowledge_base.status` — Whether the deriver is still writing

The freshness half of the stopped-writer judgement (ADR 0052, applied to the knowledge deriver by ADR 0058): the newest knowledge.derived line, the newest session.closed line, how many closed episodes no derivation names, the candidate counts, and — once a derivation has run in this checkout and the switch is on — whether the newest closed episode went underived past session.freshness.stale_minutes. Episode ids are compared, never line order. Whether the close path still calls the deriver is read from the source by the shell validator, not here.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_knowledge_status` |
| MCP resource | `majordomus://knowledge-status` |
| HTTP | `GET /api/v1/knowledge/status` |
| CLI | `majordomus knowledge status` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::knowledge_base |
| tags | knowledge, continuity, session |

Input: none.

Output: `KnowledgeStatus`.

