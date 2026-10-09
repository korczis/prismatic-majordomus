<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `intent_opposition` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.18.0 -->
# Module `intent_opposition` — Opposition

The opposition to an intent's plan, executed (ADR 0112): the structural findings the coverage, the plan and the gap review derive about one intent now, the findings a reviewer recorded with their resolutions, the one disposition both derive, and the revision of the plan a review judges against the stamp its critique carries. Reading is the brief a reviewer works from; recording stamps which plan a review was run over, and writes nothing else.

Stability: behaviorally_verified. Capabilities: 2.

## `intent_opposition.record` — Stamp the review of an intent's plan with the plan it was run over

Derive the opposition to one intent's plan and stamp its critique with what was reviewed: `reviewed_revision`, the plan's revision as derived at that moment and never one a caller supplies; `reviewed_at`, the commit; and `reviewed_with`, the tool and its version. An intent with no critique gets a record with no findings, which needs `reviewed_by`. The stamp is three top-level lines and every other line of the record is left as it is: findings are a reviewer's to write. A critique whose own findings do not hold — an unknown class or resolution, a duplicate, a rejection without a reason, a resolution planned into an issue that does not exist, serves another intent or is cancelled — is refused and nothing is written. The event `opposition.recorded` is appended to the ledger with the intent, the revision and the disposition derived then; the disposition is not written to the record. With `check`, the answer is what would be stamped and nothing is written.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_opposition_record` |
| HTTP | `POST /api/v1/intents/opposition/record` |
| CLI | `majordomus intent stamp` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intent_opposition |
| tags | intent, project, planning, lifecycle |

| input | type | required | description |
|---|---|---|---|
| `intent` | string | yes | The intent whose review is stamped, by id. |
| `check` | boolean | no | Answer what would be stamped and write nothing. |
| `reviewed_by` | string or null | no | Who reviewed, for a critique this call creates; an existing record keeps the
reviewer it names. |

Output: `OppositionRecorded`.

## `intent_opposition.review` — The opposition to one intent's plan, and the brief a reviewer works from

For one intent: its statement, invariants, non-goals and criteria with the state of their evidence; every live issue serving it with its links, dependencies, scope and required evidence; the recorded gap's conditions; `structural`, every finding the intent engine and the plan derive about it now, blocking where that derivation calls it a failure and advisory where a warning; `recorded`, every finding a reviewer wrote in its critique with its resolution, source and resolver; `disposition` — `reject` while a structural finding is blocking or a recorded blocking finding is open, `accept_with_required_changes` when recorded blocking findings are each planned or rejected with a reason, `accept` otherwise — with the findings that reject it; `reviewed_plan`, the revision of what a review judges; and `review`, the critique's stamp and whether it is `none`, `not_stamped`, `current` or `stale` against that revision. Nothing structural and no disposition is stored: both are derived on every call, with no advisor, model or network.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_opposition` |
| HTTP | `GET /api/v1/intents/opposition` |
| CLI | `majordomus intent oppose` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intent_opposition |
| tags | intent, project, planning, governance |

| input | type | required | description |
|---|---|---|---|
| `intent` | string | yes | The intent whose plan is opposed, by id. |

Output: `IntentOpposition`.

