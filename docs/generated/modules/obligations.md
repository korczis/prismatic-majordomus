<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `obligations` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `obligations` — Obligations

What a task owes before it may be called completed, and whether the evidence that discharged each obligation still describes this tree. The vocabulary is data the distribution ships and answers in any clone; the closure is read from the local half of the layer, which this process serves to the worker in front of it and never publishes. Read, never written: `majordomus evidence` records, and a second writer for one ledger would be a second account of the same events.

Stability: behaviorally_verified. Capabilities: 2.

## `obligations.closure` — What this task still owes

Every obligation the active task declared, joined with what the vocabulary says about it and with the evidence that does or does not discharge it: what is owed, what is discharged, and what has gone stale — with the recorded input hash and the tree's current one, or the recorded commit and its label, so a reader can see against what. The judgement is the one `finish` applies, reproduced rather than re-decided, and the staleness words are the repository's only four. A checkout with no task reports that, rather than reporting nothing owed.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_obligation_closure` |
| MCP resource | `majordomus://obligations/closure` |
| HTTP | `GET /api/v1/obligations/closure` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::obligations |
| tags | obligations, completion, continuity |

Input: none.

Output: `Closure`.

## `obligations.vocabulary` — Every obligation there is

The tokens a task may declare in `requires`: what each one asks of a worker, the command that discharges it, the pathspecs its evidence is hashed over, and whether its fact is remote and therefore bound to a commit rather than to a tree. Shipped data, identical in every clone, so this answers in a checkout that has never run the lifecycle.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_obligations` |
| MCP resource | `majordomus://obligations` |
| HTTP | `GET /api/v1/obligations` |
| cache | process, 2 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::obligations |
| tags | obligations, completion |

Input: none.

Output: `Vocabulary`.

