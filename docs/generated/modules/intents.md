<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `intents` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.19.1 -->
# Module `intents` — Intent

What must become true above the milestones that realise it: each intent's statement, invariants and satisfaction criteria, its stage derived from the plan's milestone status, each criterion's state derived from the evidence ledger, and its verdict derived from those criteria alone. Nothing is stored and nothing transitions; an intent added under the project model is answered by all of these without a registration anywhere.

Stability: behaviorally_verified. Capabilities: 6.

## `intents.binding` — What a piece of work is bound to before it starts

What a task asks before it starts (ADR 0111): given the issue the work executes, the intent it serves, the paths it will touch, or an exemption class with its reason, one standing — `bound` when the work serves a criterion of a live intent through links the preflight accepts, `maintenance` when its issues sit under milestones no live intent names, `exempt` when the class is one the policy declares under `intent.exemptions` and a reason was given, `refused` otherwise, each refusal with a cause: the preflight's own seven, and nothing_named, unknown_intent, intent_retired, intent_has_no_open_work, issue_outside_intent, ambiguous_intent (paths alone reached more than one intent), unknown_exemption, exemption_without_reason, exemption_names_work. The answer carries what was named, the issues and intents resolved with what each intent asks of the worker, a note when the paths lie outside the named issue's scope, and two pins a later reader compares: `plan_revision`, which moves when the intent, a link or the critique is edited, and `evidence_standing`, which moves when a served criterion's evidence changes state. Nothing is stored.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_binding` |
| HTTP | `GET /api/v1/intents/binding` |
| CLI | `majordomus intent binding` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intents |
| tags | intent, project, governance |

| input | type | required | description |
|---|---|---|---|
| `issue` | string or null | no | The issue the work executes, by id. |
| `intent` | string or null | no | The intent the work serves, by id, when the worker names it directly. |
| `paths` | string | no | Repository-relative paths the work will touch, separated by commas. |
| `exemption` | string or null | no | An exemption class the policy declares under `intent.exemptions`. |
| `because` | string or null | no | Why the exemption applies. |

Output: `IntentBinding`.

## `intents.coverage` — Which work carries which criterion, and why each issue exists

The plan read against the intents (ADR 0073): every criterion of every live intent with the live issues that serve it, the milestones they belong to, and its strength — covered, weakly covered when no serving issue requires evidence, observed when the recorded gap saw it already true, or uncovered when nothing in the plan will make it true — and every issue with the reason it exists: the criteria it serves, maintenance under a milestone no intent names, or unexplained under one that realises an intent.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_coverage` |
| HTTP | `GET /api/v1/intents/coverage` |
| CLI | `majordomus intent coverage` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intents |
| tags | intent, project, planning |

Input: none.

Output: `IntentCoverage`.

## `intents.list` — Every intent, with its derived stage and verdict

Every intent the project model declares, each with the status the plan derives for its milestones, the state of the evidence behind each satisfaction criterion, the stage those two derive — declared, planned, executing, verifying or satisfied, or cancelled or superseded when the record says so — and the verdict the criteria alone derive (ADR 0107): satisfied when every criterion is met, unsatisfied when a test or claim criterion is not, unknown when only command or deployment criteria are unmet or none is declared, with the criteria holding it back.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intents` |
| MCP resource | `majordomus://intents` |
| HTTP | `GET /api/v1/intents` |
| CLI | `majordomus intent list` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intents |
| tags | intent, project |

Input: none.

Output: `IntentList`.

## `intents.preflight` — Which intent a piece of work serves, or why it may not proceed

Given the issue a piece of work executes, or the paths it will touch, one verdict: `serves` when no issue is refused and at least one serves a criterion of a live intent through a link that holds, the plan of each such intent critiqued with no blocking finding open (issues judged maintenance beside it do not change the verdict); `maintenance` when the issues sit under milestones no live intent names and serve nothing, as `intent validate` allows; `refused` otherwise, each refusal with its issue, a cause — unknown_issue, no_issue_covers_paths, issue_serves_nothing, serves_another_intent, serves_unknown_criterion, intent_not_critiqued, open_blocking_finding — in path mode every issue judged and the worst verdict answered. The answer carries, for the intents reached and no others, the statement, the served criteria with the live state of their evidence, the invariants, non-goals and governance, the critique with its open blocking findings, and the recorded gap bounded to those criteria.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_preflight` |
| HTTP | `GET /api/v1/intents/preflight` |
| CLI | `majordomus intent preflight` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intents |
| tags | intent, project, governance |

| input | type | required | description |
|---|---|---|---|
| `issue` | string or null | no | The issue the work executes, by id. When given, the paths are not consulted. |
| `paths` | string | no | Repository-relative paths the work will touch, separated by commas. |

Output: `IntentPreflight`.

## `intents.record` — One intent, with everything derived about it

One intent in full: its statement and invariants as authored, each milestone with the status the plan derives, each satisfaction criterion with the state of its evidence and the command that reproduces it, the stage, and the verdict the criteria alone derive with the criteria holding it back. The record's own file stays at `majordomus://intent/<id>`.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_record` |
| HTTP | `GET /api/v1/intents/record` |
| CLI | `majordomus intent show` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intents |
| tags | intent, project |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The intent's id, which is also its file name under `.ai/repo/project/intents/`. |

Output: `IntentView`.

## `intents.validate` — What the intent model refuses

Every finding over the intents: an intent naming no milestone or no criterion, a milestone that does not resolve, a criterion with no evidence reference or one that names nothing this repository holds, governance that does not resolve, a successor that is not an intent, a file name that disagrees with its id — each a failure — and a milestone no intent serves, a warning.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_intent_validate` |
| HTTP | `GET /api/v1/intents/validate` |
| CLI | `majordomus intent validate` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::intents |
| tags | intent, project, validation |

Input: none.

Output: `IntentValidation`.

