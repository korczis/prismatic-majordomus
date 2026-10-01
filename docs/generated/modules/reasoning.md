<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `reasoning` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.11.0 -->
# Module `reasoning` — Reasoning

Provider-independent reasoning (ADR 0098): which optional advisors can be asked and what they offer, what a material uncertainty calls for, the session's typed reasoning records — assessments, plans, consultations, disagreements, resolutions, conclusions, validations, attempts — and the state, timeline and provenance they derive. Nothing here talks to an advisor, and nothing here needs one: with none available, the plan is a structured local review and the session concludes on its own evidence.

Stability: experimental. Capabilities: 6.

## `reasoning.advisors` — The advisors and what they can do now

Every declared advisor — then every linked mesh peer carrying the review feature — with its status (available, unavailable, not_configured, disabled, temporarily_failed, rate_limited) and why, from presence alone (an executable on PATH, a credential variable set; never a value), the reasoning mode and the recorded outcomes of earlier consultations. Plus the capacity per advisory capability. No advisor is an ordinary answer: reasoning is operational either way.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_reasoning_advisors` |
| MCP resource | `majordomus://reasoning/advisors` |
| HTTP | `GET /api/v1/reasoning/advisors` |
| CLI | `majordomus reasoning advisors` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::reasoning |
| tags | reasoning, advisors |

Input: none.

Output: `AdvisorsReport`.

## `reasoning.check` — Check that reasoning stays provider-independent

The executable half of the rules advisors-are-optional and review-is-recorded-not-claimed: the advisor catalogue resolves against the provider table and the model catalogue; every adapter it names has its transport module; no provider-independent source names an advisor; no CI workflow or gate names a model credential; no document asserts a refused claim; every stored consultation names an advisor its plan selected, and every conclusion's review matches what it cites.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_reasoning_check` |
| HTTP | `GET /api/v1/reasoning/check` |
| CLI | `majordomus reasoning check` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::reasoning |
| tags | reasoning, governance |

Input: none.

Output: `ReasoningCheck`.

## `reasoning.explain` — Why a decision was made

One reasoning record with the whole chain of its assessment: the uncertainty and its evidence, the plan and who it selected or why nobody, each consultation, each disagreement and the experiment that settled it, the conclusion and its validation — the provenance of a decision without the conversation that produced it.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_reasoning_explain` |
| HTTP | `GET /api/v1/reasoning/explain` |
| CLI | `majordomus reasoning explain` |
| cache | — |
| benchmark | waived (transient_state) |
| provenance | builtin majordomus_cli::capability::builtin::reasoning |
| tags | reasoning, provenance |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | A record id: a conclusion, usually. |

Output: `ReasoningExplanation`.

## `reasoning.plan` — What an uncertainty calls for

Decide, without recording anything, whether a stated uncertainty warrants independent review, how much the mode allows, and which available advisors a capability-driven selection would ask — with every advisor left out and why. Trivial and low uncertainty is decided locally; material uncertainty with no suitable advisor gets the structured local review instead. Pure over the catalogue, the availability and the input.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_reasoning_plan` |
| HTTP | `GET /api/v1/reasoning/plan` |
| CLI | `majordomus reasoning plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::reasoning |
| tags | reasoning, policy |

| input | type | required | description |
|---|---|---|---|
| `materiality` | object | no | `trivial`, `low`, `material`, `high` or `critical`. |
| `confidence` | object | no | `low`, `medium` or `high`. |
| `capabilities` | string or null | no | Advisory capabilities review needs, comma-separated. |

Output: `ReviewPlan`.

## `reasoning.record` — Record a reasoning step

Write one typed reasoning record of the open task under .ai/local/state/reasoning/: an assessment (with its evidence), a plan (computed here from the availability of this moment, never supplied), a consultation (only of an advisor its plan selected), a disagreement, a resolution (evidence, never a count), a conclusion (its review count computed from the consultations it cites; refused while a disagreement on its assessment is unsettled), a validation or a failed attempt. Secret shapes are redacted before anything is written.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_reasoning_record` |
| HTTP | `POST /api/v1/reasoning/records` |
| CLI | `majordomus reasoning record` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::reasoning |
| tags | reasoning, records |

| input | type | required | description |
|---|---|---|---|
| `record` | ReasoningInput | yes | The record. |

Output: `ReasoningRecorded`.

## `reasoning.status` — The session's reasoning state

The reasoning state of the open task (or of a named one, or of all): assessments and where each stands, consultations and how each ended, disagreements and what settled them, conclusions with their computed review and validation, the timeline, token totals the adapters reported, and the Markdown report a handover carries. Derived from the records alone, deterministically.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_reasoning` |
| MCP resource | `majordomus://reasoning` |
| HTTP | `GET /api/v1/reasoning` |
| CLI | `majordomus reasoning status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::reasoning |
| tags | reasoning, session |

| input | type | required | description |
|---|---|---|---|
| `task` | string or null | no | A task id, or `all`; the open task when absent. |

Output: `ReasoningStatus`.

