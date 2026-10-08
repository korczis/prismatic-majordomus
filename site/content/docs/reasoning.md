+++
title = "Reasoning"
description = "provider-independent reasoning: material uncertainty assessed with evidence, optional advisors discovered by capability and consulted through one transport contract, disagreement settled by experiment rather than count, conclusions and review recorded, and the state carried across sessions, with no advisor required"
weight = 59
[extra]
source = "docs/REASONING.md"
+++

{% raw %}

How a session turns a material engineering uncertainty into a decision it can defend:
with evidence first, with independent advisors when some are present, without them when
none are, and with a record a later session can read instead of asking again. ADR 0098 is
the decision; `apps/majordomus-cli/src/reasoning/` the model; `scripts/lib/advisors/` the
transport.

## What it is

Provider-independent reasoning. The executing agent owns every engineering decision. It
gathers evidence, states what is uncertain and how much that matters, and asks the policy
whether independent review is worth having. If it is, and advisors are available, they are
consulted; their answers are evidence the executor weighs, never verdicts. The outcome is a
typed conclusion with its evidence, the alternatives rejected, the remaining risks and a
validation plan, and then the work continues: implementation, tests, validation.

External models make review deeper. They do not decide anything, and their absence is
not a failure. A checkout with no advisor installed, no credential and no network reasons,
decides and validates exactly as before, and says that it did so locally.

## Why

Uncertainty decided silently is uncertainty nobody can audit. An advisor's opinion pasted
into a chat is gone by the next session. And a protocol written as "ask this vendor, then
that one" breaks the day a CLI is missing or a key expires. This design makes the
uncertainty, the advice and the decision explicit records, and keeps vendors out of the
logic.

## The flow

<pre class="mermaid">
flowchart TD
  E[local evidence] --&gt; A[assessment: subject, materiality, hypothesis, evidence]
  A --&gt; P{plan: policy over capabilities and availability}
  P --&gt;|trivial or low| L[decide locally]
  P --&gt;|material, no suitable advisor| R[structured local review]
  P --&gt;|a standing conclusion answers it| U[reuse the prior conclusion]
  P --&gt;|material, advisors available| C[consult the selected advisors]
  C --&gt; N[normalised consultation records]
  N --&gt;|positions diverge| D[disagreement]
  D --&gt; X[experiment: resolution with evidence]
  X --&gt; K[conclusion]
  N --&gt; K
  R --&gt; K
  L --&gt; K
  K --&gt; I[implementation]
  I --&gt; V[validation records]
  V --&gt; H[state, report, context, handover]
</pre>


## Local-first reasoning

Evidence comes before opinion, and the writer enforces it: an assessment of `material`,
`high` or `critical` uncertainty is refused without evidence references — files, tests,
ADRs, rules, commits, commands, runtime observations. When the plan selects no advisor,
it carries the structured local review the executor performs instead: state the hypothesis
and its strongest alternative, list each assumption with the evidence behind it, name what
would falsify the hypothesis, run the smallest discriminating experiment, and record the
conclusion.

## Provider independence

The policy names capabilities, never advisors. Who can provide a capability is the advisor
catalogue's answer, and whether they can provide it *now* is availability's. A test and
`reasoning check` hold that no provider-independent source file — the reasoning module, its
capability, its command, its Cockpit page, the transport's contract and driver — spells an
advisor, adapter, executable, provider, vendor or model of the catalogue.

```sh
majordomus reasoning advisors              # every advisor, its status and why; capacity per capability
majordomus reasoning advisors --format json
```

The list is the catalogue's; this page does not repeat it. Every advisor it holds is
optional.

## Availability is state

<div class="overflow-x-auto" tabindex="0">

| Status | Means |
| --- | --- |
| `available` | Presence was observed (an executable on `PATH`, a credential variable set). Whether it answers is learned by asking. |
| `unavailable` | Its executable is not installed. |
| `not_configured` | Its credential variable is not set. The value is never read by the crate. |
| `disabled` | The mode or `MAJORDOMUS_ADVISORS_DISABLE` leaves it out. |
| `temporarily_failed` | Two consecutive transient failures (timeout, unreachable, error) opened its circuit for 10 minutes; an authentication failure for an hour. |
| `rate_limited` | It answered with a rate limit; honoured for its retry-after, else 15 minutes. |

</div>


The circuit is a pure function of the recorded consultations and the clock: no state
besides the records, nothing to reset by hand. After its cooldown an advisor is available
again and marked `recovering`, and its next answer closes the circuit. A malformed or empty
answer, or a cancellation, is about one exchange and does not count against the advisor.
`MAJORDOMUS_REASONING_COOLDOWN_SECONDS` overrides every cooldown, for example after fixing a
credential.

## Consultation policy

The budget is the number of advisors a plan may select:

<div class="overflow-x-auto" tabindex="0">

| mode | trivial | low | material | high | critical |
| --- | --- | --- | --- | --- | --- |
| `ci` | 0 | 0 | 0 | 0 | 0 |
| `fast` | 0 | 0 | 0 | 1 | 1 |
| `standard` | 0 | 0 | 1 | 2 | 2 |
| `offline` | 0 | 0 | 1 | 2 | 2 |
| `strict` | 0 | 0 | 1 | 2 | 3 |

</div>


High confidence in the local hypothesis lowers it by one. `offline` admits only advisors
that run on this machine; `ci` admits none, whatever is installed. The mode is
`MAJORDOMUS_REASONING_MODE`, else `ci` on a CI runner, else the active profile's
`reasoning:`, else `standard`.

Selection is greedy and explained: each next advisor is the one covering the most
requested capabilities not yet covered, then one reached through a different adapter (two
answers from one source are one opinion twice), then declaration order — which is where a
repository's preference ("ChatGPT first, then Gemini or Codex") lives. Every advisor not
selected appears in the plan with its reason.

```sh
majordomus reasoning plan --materiality high --capabilities independent_reasoning,code_review
```

## Evidence and records

One writer, `reasoning record`, stores one JSON file per record under
`.ai/local/state/reasoning/<task>/` — checkout state, never tracked, never published.
Secret shapes and terminal control characters are removed from every string before it is
written, and the shapes removed are named in the record.

<div class="overflow-x-auto" tabindex="0">

| Kind | Holds | The writer refuses |
| --- | --- | --- |
| `assessment` | subject, materiality, confidence, hypothesis, facts, assumptions, questions, evidence, capabilities | material uncertainty without evidence |
| `plan` | the policy's decision and the availability snapshot it was made against | a plan the caller supplies: it is computed here |
| `consultation` | advisor, status, normalised conclusion, stance, assumptions, risks, actions, falsifiers, confidence, duration, usage | an advisor the plan did not select; a completed one without a conclusion; a failed one with a conclusion |
| `disagreement` | positions (the executor's, completed consultations'), divergent assumptions, the discriminating question | fewer than two positions; a position from a consultation that did not complete |
| `resolution` | the experiment, what it showed, the position it favours, its evidence | no evidence |
| `conclusion` | decision, rationale, evidence, consultations weighed, rejected alternatives, risks, validation plan; `reviewed_by` and `independent_review_count` computed | an unresolved disagreement; an answer received and not weighed; material uncertainty without evidence or validation plan |
| `validation` | the check, the command, pass or fail | |
| `attempt` | what was tried, the failure observed, the assumption it invalidated, the updated hypothesis | |

</div>


```sh
majordomus reasoning record --file assessment.json      # or JSON on standard input
```

A consultation is admitted against the snapshot of the plan that selected it, so an answer
arriving after another request opened the advisor's circuit is still the evidence it was.

## Disagreement

Advisors disagreeing with each other, or with the executor, is recorded as a disagreement:
the positions, the assumptions they divide on, and the question whose answer decides. A
conclusion on that assessment is refused until a resolution settles it, and a resolution is
refused without evidence. The number of advisors on each side never enters the decision.
Agreement is evidence about agreement; the experiment is evidence about the code.

## Sessions and handovers

`majordomus reasoning status` derives the task's state from its records: each assessment
and where it stands (open, planned, disputed, concluded, validated), each consultation, each
disagreement and its resolution, each conclusion with its computed review and validation,
a timeline, and the token totals adapters reported. The same report appears in
`majordomus context` (section REASONING) and in every `majordomus handover --derive`
(section Reasoning). A new session reads the decision and who reviewed it instead of
rediscovering it.

When a later assessment states the same subject, the plan reuses the standing conclusion
and consults nobody — unless the assessment declares `new_evidence`, in which case review
happens again and the new conclusion names the old one in `supersedes`.

```sh
majordomus reasoning explain <conclusion-id>    # the whole provenance chain of a decision
```

## The transport

Network code stays out of the crate (ADR 0032). `scripts/advisor-consult` records the plan
through the crate, consults what it selected concurrently, and records each outcome back in
the plan's order, not in the order answers arrived:

```sh
scripts/advisor-consult --assessment <id> [--question "..."] [--timeout 90] [--prior-opinion "..."]
```

Every adapter speaks one contract (`scripts/lib/advisors/contract.mjs`): typed failures
(`timeout`, `rate_limited`, `auth_failed`, `malformed`, `empty`, `unavailable`,
`cancelled`, `error`), a deadline and cancellation, and normalisation of the answer into the
consultation fields. The question carries the distilled evidence, marks the executor's
hypothesis and any prior advisor's answer as opinions rather than facts, and asks for the
strongest argument against, the hidden assumptions and a falsifying experiment. Raw answers
are not kept.

## Mesh peers

Linked mesh runtimes carrying the `reviews` feature are advisors too, discovered from the
mesh by the shared server and never listed in the catalogue. A request reaches one as a mesh
review request (ADR 0067), through the server named by `MAJORDOMUS_SERVER_URL`; its answer is
the first review answer replicated back. No peer is needed, as no advisor is.
`apps/majordomus-cli/tests/reasoning_mesh.rs` proves both halves with two real runtimes and
every catalogue advisor disabled: the linked runtime is an advisor, a code-review plan
selects it, a review sent through the mesh is answered and read back, and once the runtime
is gone the advisor disappears and the same plan decides locally.

## Offline and CI

`MAJORDOMUS_REASONING_MODE=offline` admits only advisors running on this machine; with none,
the local review path runs. A CI runner is `ci` mode: no advisor at all, and no required gate
may name a model credential — `reasoning check` refuses a workflow or gate that does. Every
reasoning test runs against fakes: scripted fixture adapters, a fake fetch, a fake process.

## Surfaces

<div class="overflow-x-auto" tabindex="0">

| Surface | Reasoning |
| --- | --- |
| CLI | `majordomus reasoning advisors|plan|record|status|explain|check` |
| HTTP | `/api/v1/reasoning/advisors`, `/api/v1/reasoning/plan`, `/api/v1/reasoning/records` (POST), `/api/v1/reasoning`, `/api/v1/reasoning/explain`, `/api/v1/reasoning/check` |
| MCP | `majordomus_reasoning_advisors`, `majordomus_reasoning_plan`, `majordomus_reasoning_record`, `majordomus_reasoning`, `majordomus_reasoning_explain`, `majordomus_reasoning_check`; resources `majordomus://reasoning`, `majordomus://reasoning/advisors` |
| Cockpit | `/cockpit/reasoning` |
| Environment | `reasoning` in `majordomus env --format json`; the `advisors` line of the banner |
| Doctor | advisor availability as information, `reasoning check` findings as failures |
| Session | the REASONING section of `majordomus context`, the Reasoning section of a derived handover |

</div>


All of them are projections of the six capabilities of the `reasoning` module; none keeps
an advisor list of its own.

## Troubleshooting

- **An advisor is `not_configured`** — its vendor's credential variable is not set in the
  environment of the process asking. In a linked worktree, the primary checkout's
  `.envrc.local` is not loaded unless it is linked there.
- **An advisor is `temporarily_failed` with `authentication_failed`** — the variable is set
  and the vendor refused it. Fix the credential, then consult with
  `MAJORDOMUS_REASONING_COOLDOWN_SECONDS=0` to try again at once.
- **A client tool exits with an error** — the diagnostic in the consultation record is the
  last line of its standard error, with control characters removed.
- **Nothing is consulted** — read the plan: every advisor left out carries its reason, and
  the mode and the materiality decide the budget.

## Adding an advisor

Declare it in `share/advisors.yaml`: an id, a title, a transport, an adapter, the provider,
vendor or model it references, the executable whose presence means it is installed, and its
capabilities. If the transport is new, add one module `scripts/lib/advisors/<adapter>.mjs`
exporting `exchange(advisor, prompt, io)`, and its cases in the contract suite. Nothing else
changes: the command line, HTTP, OpenAPI, MCP, the Cockpit, the environment and doctor all
follow, as `test/cases/735_a_new_advisor_needs_no_consumer_edit.sh` proves.
{% endraw %}
