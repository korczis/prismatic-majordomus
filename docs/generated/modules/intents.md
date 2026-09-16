<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `intents` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.7.0 -->
# Module `intents` — Intent

What must become true above the milestones that realise it: each intent's statement, invariants and satisfaction criteria, its stage derived from the plan's milestone status, and each criterion's state derived from the evidence ledger. Nothing is stored and nothing transitions; an intent added under the project model is answered by all of these without a registration anywhere.

Stability: behaviorally_verified. Capabilities: 5.

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

## `intents.list` — Every intent, with its derived stage

Every intent the project model declares, each with the status the plan derives for its milestones, the state of the evidence behind each satisfaction criterion, and the stage those two derive: declared, planned, executing, verifying or satisfied — or cancelled or superseded, when the record says so.

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

## `intents.preflight` — Which intent a piece of work serves

Given the issue a piece of work executes, or the paths it will touch, the intents it serves — issue to milestone to intent, each link named — and the governance those intents load; or a refusal naming the first link that is missing: an issue that does not exist, paths no open issue covers, a milestone no intent names.

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

One intent in full: its statement and invariants as authored, each milestone with the status the plan derives, each satisfaction criterion with the state of its evidence and the command that reproduces it, and the stage. The record's own file stays at `majordomus://intent/<id>`.

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

