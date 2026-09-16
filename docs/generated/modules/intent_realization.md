<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `intent_realization` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.7.0 -->
# Module `intent_realization` — Intent realization

Which work realises which intent: every task of this checkout's ledger with the episodes, providers and handovers that carried it, every closed session record and every claim on the peer board, joined through the plan to the intents they serve, each link marked declared, observed, derived or inferred; each intent's unmet criteria with the issues serving them; and the drift between closed work and current evidence. Derived on every read and stored nowhere, so an intent outlives every session and provider that worked on it.

Stability: behaviorally_verified. Capabilities: 2.

## `intent_realization.explain` — Why an intent stands where it stands

One intent explained from the derivations alone: a sentence for its stage naming each milestone's derived status, a sentence per criterion naming its evidence state, the plan's coverage of it and the command that reproduces it, a sentence for the work and providers realising it, and each drift finding; beside them the derived intent, the coverage of its criteria, its realization and the work linked to it.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_explain` |
| HTTP | `GET /api/v1/intents/explain` |
| CLI | `majordomus intent explain` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intent_realization |
| tags | intent, evidence, explain |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The intent's id. |

Output: `IntentExplanation`.

## `intent_realization.work` — Which work realises which intent, and what each still lacks

Every unit of work this checkout can see — ledger tasks with their episodes, providers and handovers, closed session records, peer claims — with every intent it realises through issue, milestone and criterion, each link's provenance (declared: the work cites the issue; observed: its episode moved the issue; derived: its branch names the issue; inferred: only an open issue's scope overlaps) or the first missing link; every intent with its unmet criteria and the issues serving each, the work and providers realising it, and its drift: closed_work_contradicted when every milestone is DONE and a criterion's evidence is stale or failing, closed_work_unproven when it never existed, criterion_closed_unmet when every serving issue is DONE and the criterion is not met. Live work serving no intent is a warning.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_realization` |
| HTTP | `GET /api/v1/intents/realization` |
| CLI | `majordomus intent realization` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intent_realization |
| tags | intent, session, handover, peers |

| input | type | required | description |
|---|---|---|---|
| `intent` | string or null | no | An intent id. When given, only that intent, the work linked to it and its findings are
answered; the unlinked work is left out. |

Output: `IntentRealization`.

