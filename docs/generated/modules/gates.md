<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `gates` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.3.0 -->
# Module `gates` — Completion gates

Completion as a state the repository decides rather than a claim a worker makes: the validation gates this repository declares in its CI model, which of them a task's own change set selects, what each one last reported, whether that verdict still describes the tree it was taken over, and therefore whether the task may be called finished. A gate that has never reported is `queued` and not `pass`, a run whose files have changed since is `stale` and refuses exactly as a failure does, and a gate the change cannot affect is `exempt` rather than unknown.

Stability: behaviorally_verified. Capabilities: 5.

## `gates.baseline` — A check judged against the commit the change is built on

The runs of one check over the change, over the commit it is built on (its merge base with the trunk, or the trunk's tip) and optionally over a neutral perturbation of that commit, the same declared number of times on each side, judged per failure id: `introduced` (the change shows it and the base never did), `pre_existing` (the base shows it too: the trunk's), `fixed`, or `unknown` (the base could not be compared with, or a failure that names nothing matches another that names nothing). Flaky is a property, never a class. Under the default rule, `subset` — no failure the base does not also show — only an introduced id blocks; under `rate` an id blocks when the change shows it in more runs than the base and the control by more than the declared slack. The verdict is `clean`, `clean_with_trunk_failures` (with the trunk alarm: the base's own failures and their counts), `introduced` or `undecided` with its reason, and carries its denominator and the provenance the caller supplied. A finding that names a file says whether that file is in the change set, so a whole-tree scan can say "outside your diff". It judges and never runs the check, and never turns a red verification into a green one.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_gate_baseline` |
| HTTP | `POST /api/v1/gates/baseline` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, ci, evidence, verification |

| input | type | required | description |
|---|---|---|---|
| `runs` | integer | yes | The runs declared per side: every side is compared over exactly this many. |
| `rule` | BaselineRule | no | `subset` (the default: no failure the base does not also show) or
`{"rate": {"slack": k}}`. |
| `change` | BaselineSide | yes | The runs over the change. |
| `base` | BaselineSide | yes | The runs over the commit the change is built on: its merge base with the trunk, or
the trunk's tip. |
| `control` | object | no | The runs over a neutral perturbation of the base (another seed, an empty commit): the
noise floor the `rate` rule compares against. |
| `changed` | array or null | no | The change set `in_diff` is taken over. Absent: the paths between the base commit and
the working tree of this checkout, when a finding names a file and the base names a
commit. |
| `command` | string or null | no | The command that was run, echoed into the report as its provenance. |

Output: `BaselineReport`.

## `gates.completion` — Whether this task may be called finished

The active task's own change set — from the commit it started at to the working tree, uncommitted files included — put through the CI model: which gates it selects and why, what each one last reported and over which files, which verdicts have gone stale because the tree moved underneath them, which required gates have never reported at all, and which obligations the change implies whether or not the task declared them. `finishable` is false only when a required gate is known to be failing, stale or blocked by one that is; absence of a verdict is reported as unverified rather than silently accepted. The done invariant is the questions of share/completion.yaml, in its order, each answered from the source it names; `stage` is the stage of the work those answers fold to, and `complete` is true only when every question passes or is exempt — finishable and complete are two different words.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_completion` |
| MCP resource | `majordomus://gates/completion` |
| HTTP | `GET /api/v1/gates/completion` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, completion, tasks, evidence |

| input | type | required | description |
|---|---|---|---|
| `changed` | array or null | no | The changed paths, repository-relative. Absent means the task's own change set:
everything between the commit the task started at and the working tree. |
| `base` | string or null | no | The commit to compare with, when the task's own starting commit is not wanted. |
| `on_demand` | boolean or null | no | Also plan the gates the model marks on-demand. Off by default, because a plan that
selects a gate whose runner is unavailable is a plan whose verdict never arrives. |
| `tier` | string or null | no | Plan at this tier of the model's `tiers` (ADR 0186): a gate whose lane the tier does
not run is left out, and nothing implies or requires it back. Absent plans every lane.
A model that declares no tiers, or a tier it does not declare, is refused as invalid
input, in the words `scripts/ci-plan --tier` refuses it with. |

Output: `Completion`.

## `gates.model` — The CI model

Every validation gate this repository declares — the job that runs it, the command it runs, what it proves, whether every plan selects it and whether its runner can be had on demand at all — with the path classes that decide which change selects which gate, and, derived from those classes, the pathspecs each gate's evidence is taken over.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_gates` |
| MCP resource | `majordomus://gates` |
| HTTP | `GET /api/v1/gates` |
| cache | process, 4 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, ci, completion, introspection |

Input: none.

Output: `GateModelReport`.

## `gates.policy` — The completion policy

The one definition of done: the stages of the work in order, every question a task must answer before it may be called finished, the stage each question belongs to and the source its answer is taken from — an obligation of share/obligations.yaml, a gate of the CI model, the change set, the repository's convergence or the closing readings. Read from share/completion.yaml, the file `gates.completion` takes its questions from. A token the policy names and the vocabulary lacks is reported as a problem, and a gate question this repository's CI model never answers is listed by name.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_completion_policy` |
| MCP resource | `majordomus://gates/policy` |
| HTTP | `GET /api/v1/gates/policy` |
| cache | process, 4 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, completion, policy, introspection |

Input: none.

Output: `CompletionPolicyReport`.

## `gates.state` — How each gate last went

Every gate the CI model declares with the standing of its last recorded run in this checkout, whichever task recorded it: `pass` or `fail` over the files as they stand now, `stale` when those files have changed since the run, `queued` when no run has ever reported — listed again under `never_reported`, because a missing verdict is the case an absent reader renders green — and `unknown` when a run exists and cannot be checked against this tree. Each run carries the commit it was taken at and whether that is the commit checked out now. Read offline from this checkout's own record; what it did not consult, the forge's CI runs, is named under `unreached` rather than left out.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_gate_state` |
| MCP resource | `majordomus://gates/state` |
| HTTP | `GET /api/v1/gates/state` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::gates |
| tags | gates, ci, evidence, introspection |

Input: none.

Output: `GateStateReport`.

